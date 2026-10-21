//! Owner-thread dispatch, snapshots, bounded operation history and capture receipts.
use crate::*;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::Duration,
};
use view_core::{LogicalPoint, LogicalRect, NodeId, WindowId};
use view_testing::{Harness, Input};

type Handler<M, A> = Box<dyn Fn(&M, Value) -> Result<(Option<A>, Value), Error>>;
struct Command<M, A> {
    descriptor: CommandDescriptor,
    handler: Handler<M, A>,
}
/// Worker-visible progress/cancellation; never gives a worker runtime access.
#[derive(Clone)]
pub struct TaskContext {
    cancelled: Arc<AtomicBool>,
    progress: Arc<AtomicU8>,
}
impl TaskContext {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn progress(&self, value: u8) {
        self.progress.fetch_max(value.min(99), Ordering::AcqRel);
    }
}
pub type Job = Box<dyn FnOnce(TaskContext) -> Result<Value, Error> + Send>;
type StartTask<M> = Box<dyn Fn(&M, Value) -> Result<Job, Error>>;
struct TaskCommand<M> {
    descriptor: CommandDescriptor,
    start: StartTask<M>,
}
struct Worker {
    thread: std::thread::JoinHandle<Result<Value, Error>>,
    context: TaskContext,
    result: Schema,
}
type Realizer<M, A> = Box<dyn FnMut(&mut Harness<M, A>, &Selector) -> Result<(), Error>>;
#[derive(Clone, Debug, serde::Serialize)]
pub struct LongOperation {
    pub id: u64,
    pub progress: u8,
    pub status: String,
    pub result: Option<Value>,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct Capture {
    pub window: String,
    pub revision: u64,
    pub width: u32,
    pub height: u32,
    pub provenance: String,
    pub overhead_micros: u64,
    pub rgba: Vec<u8>,
}
pub struct Session<M, A> {
    pub harness: Harness<M, A>,
    name: String,
    scopes: Vec<Scope>,
    commands: Vec<Command<M, A>>,
    tasks: Vec<TaskCommand<M>>,
    workers: HashMap<u64, Worker>,
    realizer: Option<Realizer<M, A>>,
    tests: HashMap<NodeId, String>,
    requests: HashMap<u64, (Request, Response)>,
    cache_bytes: usize,
    history: VecDeque<Value>,
    trace: VecDeque<String>,
    operations: Vec<LongOperation>,
    presentations: HashMap<String, (u64, String)>,
    captures: HashMap<String, Capture>,
    next_operation: u64,
    limit: usize,
    connected: bool,
}
impl<M, A> Session<M, A> {
    /// Session ID is host-issued and must be unique across attachments/restarts.
    /// This is routing identity, not a secret or network authentication scheme.
    pub fn new(
        name: String,
        harness: Harness<M, A>,
        scopes: Vec<Scope>,
        limit: usize,
    ) -> Result<Self, Error> {
        if name.is_empty() || name.len() > 128 || limit == 0 || limit > 4096 {
            return Err(Error::new(Code::Invalid, "session/retention limit"));
        }
        let mut result = Self {
            harness,
            name,
            scopes,
            commands: vec![],
            tasks: vec![],
            workers: HashMap::new(),
            realizer: None,
            tests: HashMap::new(),
            requests: HashMap::new(),
            cache_bytes: 0,
            history: VecDeque::new(),
            trace: VecDeque::new(),
            operations: vec![],
            presentations: HashMap::new(),
            captures: HashMap::new(),
            next_operation: 1,
            limit,
            connected: true,
        };
        result.harness.runtime_mut().observe(limit);
        Ok(result)
    }
    pub fn test_id(&mut self, node: NodeId, name: String) -> Result<(), Error> {
        if name.len() > 128 || self.tests.len() >= self.limit {
            return Err(Error::new(Code::Limit, "test IDs"));
        }
        self.tests.insert(node, name);
        Ok(())
    }
    /// Typed arguments are deserialized only after exact schema validation.
    /// Handler can read the document and produce one ordinary action; never gets mutable model access.
    pub fn register<I: DeserializeOwned + 'static>(
        &mut self,
        descriptor: CommandDescriptor,
        handler: impl Fn(&M, I) -> Result<(Option<A>, Value), Error> + 'static,
    ) -> Result<(), Error> {
        if self.commands.len() >= self.limit
            || descriptor.name.len() > 128
            || !descriptor.name.contains('.')
            || self
                .commands
                .iter()
                .any(|c| c.descriptor.name == descriptor.name)
        {
            return Err(Error::new(
                Code::Invalid,
                "duplicate, unnamespaced or oversized command",
            ));
        }
        self.commands.push(Command {
            descriptor,
            handler: Box::new(move |m, v| {
                let input = serde_json::from_value(v)
                    .map_err(|e| Error::new(Code::Invalid, e.to_string()))?;
                handler(m, input)
            }),
        });
        Ok(())
    }
    pub fn revision(&self) -> u64 {
        self.harness.runtime().snapshot().revision.get()
    }
    /// Register explicit application realization/ensure-visible setup. Not physical scrolling.
    pub fn set_realizer(
        &mut self,
        handler: impl FnMut(&mut Harness<M, A>, &Selector) -> Result<(), Error> + 'static,
    ) {
        self.realizer = Some(Box::new(handler));
    }
    /// Register bounded background work. Worker receives only owned inputs and cancellation.
    pub fn register_task<I: DeserializeOwned + 'static>(
        &mut self,
        descriptor: CommandDescriptor,
        start: impl Fn(&M, I) -> Result<Job, Error> + 'static,
    ) -> Result<(), Error> {
        if self.tasks.len() >= self.limit
            || !descriptor.name.contains('.')
            || self
                .tasks
                .iter()
                .any(|t| t.descriptor.name == descriptor.name)
        {
            return Err(Error::new(Code::Invalid, "task registration"));
        }
        self.tasks.push(TaskCommand {
            descriptor,
            start: Box::new(move |m, v| start(m, serde_json::from_value(v).map_err(json_error)?)),
        });
        Ok(())
    }
    fn pump_workers(&mut self) {
        let ready: Vec<_> = self
            .workers
            .iter()
            .filter(|(_, w)| w.thread.is_finished())
            .map(|(id, _)| *id)
            .collect();
        for (id, worker) in &self.workers {
            if let Some(op) = self.operations.iter_mut().find(|o| o.id == *id)
                && op.status == "running"
            {
                op.progress = worker.context.progress.load(Ordering::Acquire);
            }
        }
        for id in ready {
            let worker = self.workers.remove(&id).expect("ready worker");
            let result = worker
                .thread
                .join()
                .unwrap_or_else(|_| Err(Error::new(Code::Host, "worker panic")))
                .and_then(|v| {
                    worker.result.validate(&v)?;
                    if serde_json::to_vec(&v).map_err(json_error)?.len() > MAX_MESSAGE {
                        return Err(Error::new(
                            Code::Limit,
                            "worker result exceeds message budget",
                        ));
                    }
                    Ok(v)
                });
            if let Some(op) = self.operations.iter_mut().find(|o| o.id == id)
                && op.status == "running"
            {
                match result {
                    Ok(v) => {
                        op.status = "complete".into();
                        op.progress = 100;
                        op.result = Some(v);
                    }
                    Err(e) => {
                        op.status = "failed".into();
                        op.result = Some(json!({"error":e}));
                    }
                }
            }
        }
    }
    fn coherent(&self) -> Result<(), Error> {
        if self.harness.runtime().has_work() {
            Err(Error::new(
                Code::Busy,
                "commit pending; host must drain before inspection",
            ))
        } else {
            Ok(())
        }
    }
    pub fn resolve(&self, s: &Selector) -> Result<Target, Error> {
        self.coherent()?;
        let snapshot = self.harness.runtime().snapshot();
        let matches: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|n| {
                s.window.as_ref().is_none_or(|w| *w == window_id(n.window))
                    && s.name.as_ref().is_none_or(|v| n.name.as_ref() == Some(v))
                    && s.role
                        .as_ref()
                        .is_none_or(|v| n.role.as_ref().is_some_and(|r| format!("{r:?}") == *v))
                    && s.test_id
                        .as_ref()
                        .is_none_or(|v| self.tests.get(&n.id) == Some(v))
                    && s.ancestor.as_ref().is_none_or(|v| {
                        let mut parent = n.parent;
                        while let Some(p) = parent {
                            if node_id(p) == *v {
                                return true;
                            }
                            parent = snapshot
                                .nodes
                                .iter()
                                .find(|n| n.id == p)
                                .and_then(|n| n.parent);
                        }
                        false
                    })
            })
            .collect();
        match matches.as_slice() {
            [n] => Ok(Target {
                id: node_id(n.id),
                revision: self.revision(),
            }),
            [] => Err(Error::new(Code::NotFound, "selector matched no node")),
            _ => Err(Error::new(
                Code::Ambiguous,
                format!("{} matching nodes", matches.len()),
            )),
        }
    }
    fn target(&self, t: &Target) -> Result<NodeId, Error> {
        self.coherent()?;
        if t.revision != self.revision() {
            return Err(Error::new(
                Code::Stale,
                "commit changed; explicitly resolve again",
            ));
        }
        self.harness
            .runtime()
            .snapshot()
            .nodes
            .iter()
            .find(|n| node_id(n.id) == t.id)
            .map(|n| n.id)
            .ok_or_else(|| Error::new(Code::Stale, "target generation is gone"))
    }
    /// Bounded conservative sampling; every candidate is checked against actual hit routing.
    /// Narrow/irregular regions may return NotActionable, never a guessed click.
    pub fn actionable_point(&self, t: &Target) -> Result<(WindowId, LogicalPoint), Error> {
        let id = self.target(t)?;
        let rt = self.harness.runtime();
        let n = rt
            .snapshot()
            .nodes
            .iter()
            .find(|n| n.id == id)
            .expect("validated");
        if !n.enabled || n.semantic.as_ref().is_some_and(|s| s.disabled || s.hidden) {
            return Err(Error::new(Code::NotActionable, "hidden or disabled"));
        }
        let size = rt.window_size(n.window)?;
        let bounds = n
            .global_bounds
            .intersection(LogicalRect::from_xywh(0., 0., size.width, size.height))
            .ok_or_else(|| Error::new(Code::NotActionable, "outside window"))?;
        for iy in 0..17 {
            for ix in 0..17 {
                let x = if ix == 0 {
                    0.5
                } else {
                    (ix as f32 - 0.5) / 16.
                };
                let y = if iy == 0 {
                    0.5
                } else {
                    (iy as f32 - 0.5) / 16.
                };
                let p = LogicalPoint::new(
                    bounds.origin.x + bounds.size.width * x,
                    bounds.origin.y + bounds.size.height * y,
                );
                if rt.hit_test(n.window, p)?.is_some_and(|h| h.target == id) {
                    return Ok((n.window, p));
                }
            }
        }
        Err(Error::new(
            Code::NotActionable,
            "clip, overlay, visibility or hit shape excludes sampled region",
        ))
    }
    fn discovery(&self) -> Value {
        json!({"version":VERSION,"session":self.name,"operations":["discover","snapshot","inspector","resolve","action_point","command","start_task","realize","advance","wait","poll","cancel","capture","failure_bundle"],"capabilities":{"headless":true,"physical_input":false,"image_capture":!self.captures.is_empty(),"realization":self.realizer.is_some(),"eval":false,"atomic_batch":false},"scopes":self.scopes,"commands":self.commands.iter().map(|c|descriptor(&c.descriptor)).collect::<Vec<_>>(),"tasks":self.tasks.iter().map(|t|descriptor(&t.descriptor)).collect::<Vec<_>>()})
    }
    pub fn snapshot(&self) -> Result<Value, Error> {
        self.coherent()?;
        let rt = self.harness.runtime();
        if rt.snapshot().nodes.len() > 4096 {
            return Err(Error::new(Code::Limit, "snapshot node budget"));
        }
        let nodes=rt.snapshot().nodes.iter().map(|n|json!({"id":node_id(n.id),"window":window_id(n.window),"parent":n.parent.map(node_id),"key":n.key,"test_id":self.tests.get(&n.id),"role":n.role.as_ref().map(|r|format!("{r:?}")),"name":n.name,"bounds":rect(n.bounds),"global_bounds":rect(n.global_bounds),"local_clip":n.clip.map(rect),"effective_clip":rt.diagnostic_geometry(n.id).ok().and_then(|(_,clip)|clip).map(rect),"enabled":n.enabled,"transform":format!("{:?}",n.transform),"visibility":format!("{:?}",n.visibility),"resources":n.resources.iter().map(|r|format!("{r:?}")).collect::<Vec<_>>(),"semantic":n.semantic.as_ref().map(|s|json!({"value":s.value,"focused":s.focused,"disabled":s.disabled,"hidden":s.hidden,"actions":s.actions.iter().map(|a|format!("{a:?}")).collect::<Vec<_>>()}))})).collect::<Vec<_>>();
        let windows=rt.snapshot().nodes.iter().filter(|n|n.parent.is_none()).map(|n|json!({"id":window_id(n.window),"focus":rt.focused(n.window).ok().flatten().map(node_id),"capture":rt.captured(n.window).ok().flatten().map(node_id)})).collect::<Vec<_>>();
        let c = rt.counters();
        Ok(
            json!({"session":self.name,"revision":self.revision(),"nodes":nodes,"windows":windows,"counters":{"actions":c.actions,"builds":c.builds,"layouts":c.layouts,"commits":c.commits,"mounts":c.mounts,"unmounts":c.unmounts},"pending_actions":rt.pending_actions(),"pending_inputs":self.harness.pending_inputs(),"time_millis":self.harness.now().as_millis()}),
        )
    }
    pub fn start_operation(&mut self) -> Result<u64, Error> {
        if !self.connected {
            return Err(Error::new(Code::UnknownOutcome, "attachment ended"));
        }
        if self.operations.len() >= self.limit {
            return Err(Error::new(Code::Limit, "operation retention full"));
        }
        let id = self.next_operation;
        self.next_operation = self
            .next_operation
            .checked_add(1)
            .ok_or_else(|| Error::new(Code::Limit, "IDs exhausted"))?;
        self.operations.push(LongOperation {
            id,
            progress: 0,
            status: "running".into(),
            result: None,
        });
        Ok(id)
    }
    pub fn progress(&mut self, id: u64, progress: u8) -> Result<(), Error> {
        let op = self
            .operations
            .iter_mut()
            .find(|o| o.id == id)
            .ok_or_else(|| Error::new(Code::NotFound, "operation"))?;
        if op.status != "running" {
            return Err(Error::new(Code::Cancelled, "terminal operation"));
        }
        if progress > 100 || progress < op.progress {
            return Err(Error::new(Code::Invalid, "nonmonotonic progress"));
        }
        op.progress = progress;
        Ok(())
    }
    pub fn finish_operation(&mut self, id: u64, result: Value) -> Result<(), Error> {
        if serde_json::to_vec(&result).map_err(json_error)?.len() > MAX_MESSAGE {
            return Err(Error::new(Code::Limit, "result size"));
        }
        self.progress(id, 100)?;
        let op = self
            .operations
            .iter_mut()
            .find(|o| o.id == id)
            .expect("validated");
        op.status = "complete".into();
        op.result = Some(result);
        Ok(())
    }
    /// Disconnect never implies successful cancellation of already dispatched work.
    pub fn disconnect(&mut self) {
        self.connected = false;
        for worker in self.workers.values() {
            worker.context.cancelled.store(true, Ordering::Release);
        }
        for op in &mut self.operations {
            if op.status == "running" {
                op.status = "unknown_outcome".into();
            }
        }
    }
    pub fn publish_presentation(
        &mut self,
        window: WindowId,
        revision: u64,
        boundary: &str,
    ) -> Result<(), Error> {
        if ![
            "offscreen_readback",
            "surface_present",
            "compositor_observed",
        ]
        .contains(&boundary)
        {
            return Err(Error::new(Code::Invalid, "presentation boundary"));
        }
        self.coherent()?;
        if revision > self.revision()
            || !self
                .harness
                .runtime()
                .snapshot()
                .nodes
                .iter()
                .any(|n| n.window == window)
        {
            return Err(Error::new(Code::Stale, "presentation window/revision"));
        }
        let key = window_id(window);
        if self
            .presentations
            .get(&key)
            .is_some_and(|(r, _)| *r > revision)
        {
            return Err(Error::new(Code::Stale, "presentation regressed"));
        }
        self.presentations.insert(key, (revision, boundary.into()));
        Ok(())
    }
    pub fn publish_capture(&mut self, capture: Capture) -> Result<(), Error> {
        let size = (capture.width as usize)
            .checked_mul(capture.height as usize)
            .and_then(|s| s.checked_mul(4));
        if capture.width == 0
            || capture.height == 0
            || size != Some(capture.rgba.len())
            || capture.rgba.len() > 4 * 1024 * 1024
            || (self.captures.len() >= 8 && !self.captures.contains_key(&capture.window))
        {
            return Err(Error::new(Code::Limit, "capture budget/dimensions"));
        }
        if !self
            .presentations
            .get(&capture.window)
            .is_some_and(|(r, b)| *r == capture.revision && *b == capture.provenance)
        {
            return Err(Error::new(
                Code::Stale,
                "capture lacks matching presentation receipt",
            ));
        }
        self.captures.insert(capture.window.clone(), capture);
        Ok(())
    }
    pub fn record_boundary(&mut self, kind: &str, fixture: Value) -> Result<(), Error> {
        if !["asset", "worker", "network", "clock", "seed", "environment"].contains(&kind)
            || serde_json::to_vec(&fixture).map_err(json_error)?.len() > 8192
        {
            return Err(Error::new(Code::Limit, "replay boundary kind/size"));
        }
        self.record(json!({"boundary":kind,"fixture":fixture}));
        Ok(())
    }
    fn record(&mut self, value: Value) {
        // Snapshot/capture/bundle replies must not recursively copy previous bundles
        // or multiply large pixels into the history retention budget.
        let value = if serde_json::to_vec(&value).is_ok_and(|v| v.len() <= 8192) {
            value
        } else {
            json!({"omitted":"record exceeded 8192 byte diagnostic budget"})
        };
        if self.history.len() == self.limit {
            self.history.pop_front();
        }
        self.history.push_back(value);
    }
    pub fn execute(&mut self, request: Request) -> Response {
        self.pump_workers();
        let id = request.id;
        if request.version != VERSION || request.session != self.name || !self.connected {
            return Response {
                id,
                revision: self.revision(),
                result: Err(Error::new(
                    Code::UnknownOutcome,
                    "incompatible or ended session; do not retry a mutation in a new session",
                )),
            };
        }
        if let Some((prior, response)) = self.requests.get(&id) {
            return if prior == &request {
                response.clone()
            } else {
                Response {
                    id,
                    revision: self.revision(),
                    result: Err(Error::new(
                        Code::Invalid,
                        "request ID reused with different payload",
                    )),
                }
            };
        }
        if self.requests.len() >= self.limit
            || self.cache_bytes > 64 * 1024 * 1024 - 2 * MAX_MESSAGE
        {
            return Response {
                id,
                revision: self.revision(),
                result: Err(Error::new(
                    Code::Limit,
                    "request retention full; start a new explicit session",
                )),
            };
        }
        let request_bytes = serde_json::to_vec(&request).map_or(usize::MAX, |v| v.len());
        if request_bytes > MAX_MESSAGE {
            return Response {
                id,
                revision: self.revision(),
                result: Err(Error::new(Code::Limit, "request bytes")),
            };
        }
        let result = self.dispatch(&request.operation);
        let mut response = Response {
            id,
            revision: self.revision(),
            result,
        };
        let bytes = serde_json::to_vec(&response).map_or(MAX_MESSAGE, |v| v.len());
        if bytes > 64 * 1024 * 1024 - self.cache_bytes - request_bytes {
            response.result = Err(Error::new(
                Code::Limit,
                "response exceeds retained byte budget",
            ));
        }
        self.cache_bytes +=
            request_bytes + serde_json::to_vec(&response).map_or(MAX_MESSAGE, |v| v.len());
        // No eviction: a repeated mutation ID must never silently execute again.
        self.requests
            .insert(id, (request.clone(), response.clone()));
        self.record(json!({"request":request,"response":response}));
        for trace in self.harness.runtime_mut().drain_trace() {
            if self.trace.len() == self.limit {
                self.trace.pop_front();
            }
            self.trace.push_back(format!("{trace:?}"));
        }
        response
    }
    fn dispatch(&mut self, operation: &Operation) -> Result<Value, Error> {
        if !self.scopes.contains(&Scope::Inspect) {
            return Err(Error::new(Code::Unauthorized, "inspect scope required"));
        }
        match operation {
            Operation::Discover => Ok(self.discovery()),
            Operation::Snapshot => self.snapshot(),
            Operation::Inspector => {
                let mut snapshot = self.snapshot()?;
                if let Some(nodes) = snapshot["nodes"].as_array_mut() {
                    for (index, node) in nodes.iter_mut().enumerate() {
                        if index >= 64 {
                            node["actionable_error"] = json!(Error::new(
                                Code::Limit,
                                "inspector hit sampling budget; query target explicitly"
                            ));
                            continue;
                        }
                        let target = Target {
                            id: node["id"].as_str().unwrap_or_default().into(),
                            revision: self.revision(),
                        };
                        match self.actionable_point(&target) {
                            Ok((_, point)) => node["actionable_point"] = json!([point.x, point.y]),
                            Err(error) => node["actionable_error"] = json!(error),
                        }
                    }
                }
                let trace: Vec<_> = self.trace.iter().cloned().collect();
                Ok(
                    json!({"html":crate::inspector::html(&snapshot,&trace),"snapshot":snapshot,"why_updated":trace}),
                )
            }
            Operation::Resolve { selector } => {
                serde_json::to_value(self.resolve(selector)?).map_err(json_error)
            }
            Operation::ActionPoint { target } => {
                let (w, p) = self.actionable_point(target)?;
                Ok(
                    json!({"window":window_id(w),"revision":self.revision(),"point":[p.x,p.y],"path":"geometry query; no physical input"}),
                )
            }
            Operation::Command {
                name,
                arguments,
                target,
            } => {
                let node = self.target(target)?;
                let command = self
                    .commands
                    .iter()
                    .find(|c| c.descriptor.name == *name)
                    .ok_or_else(|| Error::new(Code::Unsupported, "command"))?;
                if !self.scopes.contains(&command.descriptor.scope) {
                    return Err(Error::new(Code::Unauthorized, "command scope"));
                }
                command.descriptor.arguments.validate(arguments)?;
                let (action, result) = (command.handler)(self.harness.model(), arguments.clone())?;
                command.descriptor.result.validate(&result)?;
                if serde_json::to_vec(&result).map_err(json_error)?.len() > MAX_MESSAGE {
                    return Err(Error::new(
                        Code::Limit,
                        "command result bytes; action not dispatched",
                    ));
                }
                if action.is_some() && command.descriptor.scope == Scope::Inspect {
                    return Err(Error::new(
                        Code::Unauthorized,
                        "inspect command produced mutation",
                    ));
                }
                if let Some(action) = action {
                    self.harness
                        .schedule(Duration::ZERO, Input::Action(node, action))
                        .map_err(|_| Error::new(Code::Busy, "input queue full"))?;
                    let step = self.harness.step().map_err(|error| {
                        Error::new(
                            Code::UnknownOutcome,
                            format!(
                                "dispatch failed after scheduling; inspect before retry: {error:?}"
                            ),
                        )
                    })?;
                    if step.backpressure
                        || !step.rejected.is_empty()
                        || step.flush.backpressure
                        || !step.flush.rejected.is_empty()
                    {
                        return Err(Error::new(
                            Code::UnknownOutcome,
                            "dispatch did not finish; inspect before retry",
                        ));
                    }
                }
                Ok(result)
            }
            Operation::StartTask {
                name,
                arguments,
                target,
            } => {
                self.target(target)?;
                if self.workers.len() >= 8 {
                    return Err(Error::new(Code::Limit, "active worker budget"));
                }
                let task = self
                    .tasks
                    .iter()
                    .find(|t| t.descriptor.name == *name)
                    .ok_or_else(|| Error::new(Code::Unsupported, "task"))?;
                if !self.scopes.contains(&task.descriptor.scope) {
                    return Err(Error::new(Code::Unauthorized, "task scope"));
                }
                task.descriptor.arguments.validate(arguments)?;
                let job = (task.start)(self.harness.model(), arguments.clone())?;
                let result = task.descriptor.result.clone();
                let id = self.start_operation()?;
                let context = TaskContext {
                    cancelled: Arc::new(AtomicBool::new(false)),
                    progress: Arc::new(AtomicU8::new(0)),
                };
                let worker_context = context.clone();
                let thread = std::thread::spawn(move || job(worker_context));
                self.workers.insert(
                    id,
                    Worker {
                        thread,
                        context,
                        result,
                    },
                );
                Ok(json!({"operation":id}))
            }
            Operation::Realize { selector } => {
                if !self.scopes.contains(&Scope::Test) {
                    return Err(Error::new(Code::Unauthorized, "test setup scope"));
                }
                let realize = self
                    .realizer
                    .as_mut()
                    .ok_or_else(|| Error::new(Code::Unsupported, "no host realization provider"))?;
                realize(&mut self.harness, selector)?;
                self.harness.step()?;
                Ok(
                    json!({"target":self.resolve(selector)?,"provenance":"semantic setup; not physical scrolling"}),
                )
            }
            Operation::Advance { millis } => {
                if !self.scopes.contains(&Scope::Test) {
                    return Err(Error::new(Code::Unauthorized, "test clock scope"));
                }
                self.harness
                    .advance(Duration::from_millis(*millis))
                    .map_err(|_| Error::new(Code::Limit, "clock exhausted"))?;
                self.harness.step()?;
                Ok(json!({"time_millis":self.harness.now().as_millis()}))
            }
            Operation::Wait { predicate, turns } => {
                if *turns > 1024 {
                    return Err(Error::new(Code::Limit, "wait turn budget"));
                }
                for i in 0..=*turns {
                    if self.matches(predicate)? {
                        return Ok(json!({"matched":true,"turns":i}));
                    }
                    if i < *turns {
                        self.harness.step()?;
                    }
                }
                Err(Error::new(
                    Code::Timeout,
                    format!(
                        "predicate not reached at revision {}, pending inputs {}",
                        self.revision(),
                        self.harness.pending_inputs()
                    ),
                ))
            }
            Operation::Poll { operation } => serde_json::to_value(
                self.operations
                    .iter()
                    .find(|o| o.id == *operation)
                    .ok_or_else(|| Error::new(Code::NotFound, "operation"))?,
            )
            .map_err(json_error),
            Operation::Cancel { operation } => {
                if !self.scopes.contains(&Scope::Mutate) {
                    return Err(Error::new(Code::Unauthorized, "cancel scope"));
                }
                let op = self
                    .operations
                    .iter_mut()
                    .find(|o| o.id == *operation)
                    .ok_or_else(|| Error::new(Code::NotFound, "operation"))?;
                if op.status == "running" {
                    op.status = "cancelled".into();
                }
                if let Some(worker) = self.workers.get(operation) {
                    worker.context.cancelled.store(true, Ordering::Release);
                }
                serde_json::to_value(op).map_err(json_error)
            }
            Operation::Capture { window, revision } => {
                let c = self.captures.get(window).ok_or_else(|| {
                    Error::new(Code::Unsupported, "no image capture provider/receipt")
                })?;
                if c.revision != *revision {
                    return Err(Error::new(Code::Stale, "capture revision"));
                }
                serde_json::to_value(c).map_err(json_error)
            }
            Operation::FailureBundle => Ok(
                json!({"snapshot":self.snapshot()?,"history":self.history,"trace":self.trace,"operations":self.operations,"captures":self.captures.values().map(|c|json!({"window":c.window,"revision":c.revision,"provenance":c.provenance,"bytes":c.rgba.len(),"overhead_micros":c.overhead_micros})).collect::<Vec<_>>(),"replay":"requires recorded fixture boundaries; not arbitrary live IO"}),
            ),
        }
    }
    fn matches(&self, p: &Predicate) -> Result<bool, Error> {
        Ok(match p {
            Predicate::Commit { revision } => self.revision() >= *revision,
            Predicate::Presentation {
                window,
                revision,
                boundary,
            } => self
                .presentations
                .get(window)
                .is_some_and(|(r, b)| r >= revision && b == boundary),
            Predicate::Exists { selector } => match self.resolve(selector) {
                Ok(_) => true,
                Err(e) if e.code == Code::NotFound || e.code == Code::Busy => false,
                Err(e) => return Err(e),
            },
            Predicate::Idle => {
                !self.harness.runtime().has_work()
                    && self.harness.pending_inputs() == 0
                    && self.operations.iter().all(|o| o.status != "running")
            }
        })
    }
}
fn rect(r: LogicalRect) -> [f32; 4] {
    [r.origin.x, r.origin.y, r.size.width, r.size.height]
}
fn descriptor(d: &CommandDescriptor) -> Value {
    json!({"name":d.name,"description":d.description,"scope":d.scope,"inputSchema":d.arguments.json_schema(),"outputSchema":d.result.json_schema()})
}
impl<M, A> Drop for Session<M, A> {
    fn drop(&mut self) {
        for worker in self.workers.values() {
            worker.context.cancelled.store(true, Ordering::Release);
        }
    }
}
fn json_error(e: serde_json::Error) -> Error {
    Error::new(Code::Invalid, e.to_string())
}

/// Replay is driven through the same dispatcher; external boundaries must be supplied
/// by the caller before their corresponding requests. Missing fixtures are errors.
pub fn replay<M, A>(
    session: &mut Session<M, A>,
    records: &[Value],
    mut boundary: impl FnMut(&str, &Value, &mut Session<M, A>) -> Result<(), Error>,
) -> Result<Vec<Response>, Error> {
    if records.len() > 4096 {
        return Err(Error::new(Code::Limit, "replay record budget"));
    }
    let mut responses = vec![];
    for record in records {
        if let Some(kind) = record.get("boundary").and_then(Value::as_str) {
            boundary(kind, &record["fixture"], session)?;
        } else if let Some(request) = record.get("request") {
            let mut request: Request =
                serde_json::from_value(request.clone()).map_err(json_error)?;
            request.session = session.name.clone();
            responses.push(session.execute(request));
        } else {
            return Err(Error::new(Code::Invalid, "unknown replay record"));
        }
    }
    Ok(responses)
}

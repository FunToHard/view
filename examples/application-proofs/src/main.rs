fn main() -> view_application_proofs::Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args == ["--stress"] {
        return view_application_proofs::performance::run();
    }
    #[cfg(all(feature = "desktop-test", windows))]
    if args == ["--desktop-ime", "--selected-desktop"] {
        return view_application_proofs::native::run_japanese();
    }
    #[cfg(all(feature = "desktop-test", windows))]
    if args == ["--desktop-test", "--selected-desktop"] {
        return view_application_proofs::native::run_mode(false, true);
    }
    if args.first().is_some_and(|a| a == "--gpu") {
        let backend = match args.get(1).map(String::as_str).unwrap_or("dx12") {
            "dx12" => view::render::wgpu::Backends::DX12,
            "vulkan" => view::render::wgpu::Backends::VULKAN,
            "metal" => view::render::wgpu::Backends::METAL,
            _ => return Err("expected dx12, vulkan or metal".into()),
        };
        return view_application_proofs::presentation::gpu_proof(backend);
    }
    #[cfg(feature = "native")]
    if args
        .first()
        .is_some_and(|a| a == "--run-hidden" || a == "--show")
    {
        return view_application_proofs::native::run(args[0] == "--run-hidden");
    }
    if !args.is_empty() && args[0] != "--headless" {
        return Err(
            "expected --headless, --gpu <backend>, or native feature with --run-hidden / --show"
                .into(),
        );
    }
    let mut app = view_application_proofs::Proofs::new()?;
    app.verify()?;
    println!(
        "M4 editor/storefront/IDE contracts passed; native rendering is a separate explicit mode."
    );
    Ok(())
}

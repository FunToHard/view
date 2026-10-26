use view_text::*;

fn document(paragraphs: Vec<RichParagraph>) -> RichDocument {
    RichDocument::new(paragraphs, RichDocumentConfig::default()).unwrap()
}
fn line(range: std::ops::Range<usize>, height: f64) -> MeasuredDocumentLine {
    MeasuredDocumentLine { range, height }
}
fn settings() -> PageSettings {
    PageSettings {
        height: 40.0,
        margin_top: 5.0,
        margin_bottom: 5.0,
        ..PageSettings::default()
    }
}

#[test]
fn exact_page_boundary_splits_only_between_measured_lines() {
    let document = document(vec![
        RichParagraph::plain("abcd"),
        RichParagraph::plain("ef"),
    ]);
    let measured = DocumentMeasurements {
        revision: document.revision(),
        paragraphs: vec![
            vec![
                line(0..1, 10.0),
                line(1..2, 10.0),
                line(2..3, 10.0),
                line(3..4, 10.0),
            ],
            vec![line(0..2, 10.0)],
        ],
    };
    let pages = document.paginate(&measured, settings()).unwrap();
    assert_eq!(pages.pages.len(), 2);
    assert_eq!(
        pages.pages[0].fragments,
        vec![PageFragment {
            paragraph: 0,
            lines: 0..3,
            y: 5.0,
            height: 30.0
        }]
    );
    assert_eq!(
        pages.pages[1].fragments,
        vec![
            PageFragment {
                paragraph: 0,
                lines: 3..4,
                y: 5.0,
                height: 10.0
            },
            PageFragment {
                paragraph: 1,
                lines: 0..1,
                y: 15.0,
                height: 10.0
            }
        ]
    );
    assert_eq!(pages.revision, document.revision());
}

#[test]
fn keep_together_page_breaks_and_spacing_have_no_phantom_blank_pages() {
    let document = document(vec![
        RichParagraph {
            format: ParagraphFormat {
                space_before: 2.0,
                space_after: 3.0,
                page_break_before: true,
                ..ParagraphFormat::default()
            },
            ..RichParagraph::plain("a")
        },
        RichParagraph {
            format: ParagraphFormat {
                keep_together: true,
                space_before: 4.0,
                space_after: 100.0,
                ..ParagraphFormat::default()
            },
            ..RichParagraph::plain("bc")
        },
        RichParagraph {
            format: ParagraphFormat {
                page_break_before: true,
                ..ParagraphFormat::default()
            },
            ..RichParagraph::plain("d")
        },
    ]);
    let measured = DocumentMeasurements {
        revision: document.revision(),
        paragraphs: vec![
            vec![line(0..1, 10.0)],
            vec![line(0..1, 10.0), line(1..2, 10.0)],
            vec![line(0..1, 10.0)],
        ],
    };
    let pages = document.paginate(&measured, settings()).unwrap();
    assert_eq!(pages.pages.len(), 3);
    assert_eq!(pages.pages[0].fragments[0].y, 7.0);
    assert_eq!(
        pages.pages[1].fragments,
        vec![PageFragment {
            paragraph: 1,
            lines: 0..2,
            y: 9.0,
            height: 20.0
        }]
    );
    assert_eq!(pages.pages[2].fragments[0].y, 5.0);
}

#[test]
fn spacing_applies_once_across_a_split_and_empty_paragraph_consumes_a_line() {
    let document = document(vec![
        RichParagraph {
            format: ParagraphFormat {
                space_before: 10.0,
                ..ParagraphFormat::default()
            },
            ..RichParagraph::plain("abc")
        },
        RichParagraph::default(),
    ]);
    let measured = DocumentMeasurements {
        revision: document.revision(),
        paragraphs: vec![
            vec![line(0..1, 10.0), line(1..2, 10.0), line(2..3, 10.0)],
            vec![line(0..0, 10.0)],
        ],
    };
    let pages = document.paginate(&measured, settings()).unwrap();
    assert_eq!(pages.pages.len(), 2);
    assert_eq!(pages.pages[0].fragments[0].lines, 0..2);
    assert_eq!(pages.pages[0].fragments[0].y, 15.0);
    assert_eq!(pages.pages[1].fragments[0].y, 5.0);
    assert_eq!(pages.pages[1].fragments[1].paragraph, 1);
}

#[test]
fn stale_incomplete_overlapping_and_unicode_split_measurements_reject() {
    let mut document = document(vec![RichParagraph::plain("e\u{301}😀")]);
    let revision = document.revision();
    for lines in [
        vec![],
        vec![line(0..1, 10.0), line(1..7, 10.0)],
        vec![line(0..3, 10.0)],
        vec![line(0..3, 10.0), line(2..7, 10.0)],
        vec![line(0..0, 10.0), line(0..7, 10.0)],
        vec![line(0..7, f64::NAN)],
        vec![line(0..7, 0.0)],
        vec![line(0..8, 10.0)],
    ] {
        let measured = DocumentMeasurements {
            revision,
            paragraphs: vec![lines],
        };
        assert_eq!(
            document.paginate(&measured, settings()),
            Err(PaginationError::InvalidMeasurements)
        );
    }
    let measured = DocumentMeasurements {
        revision,
        paragraphs: vec![vec![line(0..7, 10.0)]],
    };
    document
        .apply(
            revision,
            vec![RichOperation::Style {
                paragraph: 0,
                range: 0..7,
                style: RichStyle {
                    bold: true,
                    ..RichStyle::default()
                },
            }],
        )
        .unwrap();
    assert_eq!(
        document.paginate(&measured, settings()),
        Err(PaginationError::StaleRevision)
    );
}

#[test]
fn page_and_line_limits_and_invalid_metrics_reject_without_document_effects() {
    let document = document(vec![RichParagraph::plain("ab")]);
    let measured = DocumentMeasurements {
        revision: document.revision(),
        paragraphs: vec![vec![line(0..1, 20.0), line(1..2, 20.0)]],
    };
    for limits in [
        PageSettings {
            max_pages: 1,
            ..settings()
        },
        PageSettings {
            max_pages: 0,
            ..settings()
        },
        PageSettings {
            max_lines: 1,
            ..settings()
        },
    ] {
        assert_eq!(
            document.paginate(&measured, limits),
            Err(PaginationError::Limit)
        );
    }
    for invalid in [
        PageSettings {
            height: f64::INFINITY,
            ..settings()
        },
        PageSettings {
            margin_top: -1.0,
            ..settings()
        },
        PageSettings {
            height: 10.0,
            ..settings()
        },
        PageSettings {
            margin_bottom: f64::NAN,
            ..settings()
        },
    ] {
        assert_eq!(
            document.paginate(&measured, invalid),
            Err(PaginationError::InvalidSettings)
        );
    }
    assert!(!document.can_undo());
    assert_eq!(document.paragraphs()[0].text(), "ab");
}

#[test]
fn oversized_line_and_keep_together_paragraph_reject_explicitly() {
    for (keep, heights, before) in [
        (false, vec![31.0], 0.0),
        (true, vec![20.0, 20.0], 0.0),
        (false, vec![20.0], 11.0),
    ] {
        let content = "x".repeat(heights.len());
        let document = document(vec![RichParagraph {
            format: ParagraphFormat {
                keep_together: keep,
                space_before: before,
                ..ParagraphFormat::default()
            },
            ..RichParagraph::plain(content)
        }]);
        let measured = DocumentMeasurements {
            revision: document.revision(),
            paragraphs: vec![
                heights
                    .into_iter()
                    .enumerate()
                    .map(|(index, height)| line(index..index + 1, height))
                    .collect(),
            ],
        };
        assert_eq!(
            document.paginate(&measured, settings()),
            Err(PaginationError::UnbreakableContent)
        );
    }
}

#[test]
fn mixed_rtl_lines_preserve_logical_source_coverage() {
    let document = document(vec![RichParagraph::plain("אבabc")]);
    let measured = DocumentMeasurements {
        revision: document.revision(),
        paragraphs: vec![vec![line(0..4, 20.0), line(4..7, 20.0)]],
    };
    let pages = document.paginate(&measured, settings()).unwrap();
    assert_eq!(pages.pages.len(), 2);
    assert_eq!(pages.pages[0].fragments[0].lines, 0..1);
    assert_eq!(pages.pages[1].fragments[0].lines, 1..2);
}

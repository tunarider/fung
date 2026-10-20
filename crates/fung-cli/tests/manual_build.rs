//! The manual: built from `manual/` and compared with `docs/`.

mod common;
mod manual_site;

use std::collections::{BTreeMap, HashMap};

use manual_site::examples::{Outcome, run_all};
use manual_site::markup;
use manual_site::render::{Ctx, Version, page_html};
use manual_site::site;
use manual_site::toc::{self, Entry};

/// Runs the one example of a manuscript body.
fn run_one(body: &str) -> Result<Outcome, Vec<String>> {
    let page = markup::parse("t.md", &format!("# T\n\n{body}")).unwrap();
    let ex = markup::examples(&page);
    assert_eq!(ex.len(), 1, "{body}");
    run_all(&common::fung_bin(), &ex).map(|mut m| m.remove("t.md:1").unwrap())
}

fn ok(output: &str) -> Outcome {
    Outcome {
        output: output.into(),
        code: 0,
    }
}

#[test]
fn run_output_is_captured_in_order() {
    let got = run_one("```fung run\nprint(\"a\")\neprint(\"b\")\nprint(\"c\")\n```\n");
    assert_eq!(got, Ok(ok("a\nb\nc\n")));
}

#[test]
fn fixtures_args_and_stdin() {
    let got = run_one(
        "```file in.txt\nx\ny\n```\n\n```fung run args=\"1 2\" stdin=in.txt env=\"WHO=fung\"\n\
         import fs\n\nprint(args)\nprint(fs.read_lines(\"in.txt\")!)\nprint(read_line()!)\nprint(env(\"WHO\"))\n```\n",
    );
    assert_eq!(got, Ok(ok("[\"1\", \"2\"]\n[\"x\", \"y\"]\nx\nfung\n")));
}

#[test]
fn exit_code_must_match() {
    let got = run_one("```fung run exit=1 error=E0606\nparse_int(\"abc\")\n```\n").unwrap();
    assert_eq!(got.code, 1);
    assert!(got.output.contains("E0606"), "{}", got.output);
    let e = run_one("```fung run\nparse_int(\"abc\")\n```\n").unwrap_err();
    assert!(e[0].starts_with("t.md:1") && e[0].contains("exit"), "{e:?}");
}

#[test]
fn fixtures_may_be_in_directories() {
    let got = run_one(
        "```file logs/api.log\nGET /\n```\n\n```fung run\nimport fs\n\nprint(fs.read_lines(\"logs/api.log\")!)\n```\n",
    );
    assert_eq!(got, Ok(ok("[\"GET /\"]\n")));
    // a fixture never lands outside the example's directory
    for bad in ["../x.txt", "/tmp/x.txt"] {
        let e = run_one(&format!(
            "```file {bad}\nx\n```\n\n```fung run\nprint(1)\n```\n"
        ))
        .unwrap_err();
        assert!(e[0].contains(bad), "{e:?}");
    }
}

#[test]
fn sandbox_paths_are_dot() {
    let got = run_one(
        "```file in.txt\nx\n```\n\n```fung run\nimport path\n\nprint(path.abs(\"in.txt\"))\n```\n",
    );
    assert_eq!(got, Ok(ok("./in.txt\n")));
}

#[test]
fn nondeterministic_output_is_refused() {
    let src = "import time\n\nprint(time.now())\n```\n";
    let e = run_one(&format!("```fung run\n{src}")).unwrap_err();
    assert!(e[0].contains("hide-output"), "{e:?}");
    assert!(run_one(&format!("```fung run hide-output\n{src}")).is_ok());
}

#[test]
fn check_and_fmt_failures_are_reported() {
    let e = run_one("```fung run\nprint(nope)\n```\n").unwrap_err();
    assert!(e[0].contains("check"), "{e:?}");
    let e = run_one("```fung run\nx=1\n```\n").unwrap_err();
    assert!(e[0].contains("fmt"), "{e:?}");
    // a check example is checked but not run
    let e = run_one("```fung check\nx=1\n```\n").unwrap_err();
    assert!(e[0].contains("fmt"), "{e:?}");
    assert!(run_one("```fung check\nexit(3)\n```\n").is_ok());
}

#[test]
fn the_environment_is_fixed() {
    // the host's home and temporary directory never reach a page
    let got = run_one(
        "```fung run\nprint(env(\"HOME\"))\nprint(env(\"TMPDIR\"))\nprint(env(\"TZ\"))\n```\n",
    );
    assert_eq!(got, Ok(ok(".\n.\nUTC\n")));
}

#[test]
fn each_run_starts_from_the_fixtures() {
    // an example that writes a file is not "nondeterministic"
    let got = run_one(
        "```fung run\nimport fs\n\nprint(fs.exists(\"out.txt\"))\nfs.write(\"out.txt\", \"x\")!\n```\n",
    );
    assert_eq!(got, Ok(ok("false\n")));
}

#[test]
fn compile_errors_can_be_shown() {
    // an example that states its failing exit may fail `check` (and the
    // formatter, which cannot parse it); its run shows the real error
    let got = run_one("```fung run exit=2 error=E0110\nx = )\n```\n").unwrap();
    assert_eq!(got.code, 2);
    assert!(got.output.contains("error[E0110]"), "{}", got.output);
    let got = run_one("```fung run exit=2 error=E0204\nprint(nope)\n```\n").unwrap();
    assert!(got.output.contains("E0204"), "{}", got.output);
    // without `exit=`, a failing check still stops it
    assert!(run_one("```fung run\nprint(nope)\n```\n").is_err());
    // and an unformatted example that parses is still refused
    let e = run_one("```fung run exit=1 error=E0803\nx=1\nfail(\"x\")\n```\n").unwrap_err();
    assert!(e[0].contains("fmt"), "{e:?}");
}

#[test]
fn a_failing_example_names_the_error_it_shows() {
    // the error it shows must be the one it says (a missing fixture shows
    // E0301, not the error the example is about)
    let e = run_one("```fung run exit=2 error=E0304\nimport \"./lib\"\n\nprint(lib._x())\n```\n")
        .unwrap_err();
    assert!(e[0].contains("E0301") && e[0].contains("E0304"), "{e:?}");
    // an example that shows an error must say which
    let e = run_one("```fung run exit=2\nx = )\n```\n").unwrap_err();
    assert!(e[0].contains("error=E0110"), "{e:?}");
    // a failing exit without an error message needs no `error=`
    let got = run_one("```fung run exit=3\nexit(3)\n```\n").unwrap();
    assert_eq!(got.code, 3);
    let e = run_one("```fung run exit=3 error=E0101\nexit(3)\n```\n").unwrap_err();
    assert!(e[0].contains("E0101"), "{e:?}");
}

#[test]
fn dump_runs_the_stage() {
    let got = run_one("```dump bytecode\nfn double(x) = x * 2\n```\n").unwrap();
    assert!(
        got.output.contains("== double ==") && got.output.contains("MUL"),
        "{}",
        got.output
    );
}

// ---- the contents file and the page frame ----

const FIXTURE: &str = "crates/fung-cli/tests/data/manual";

fn latest() -> Version {
    Version {
        dir: "latest".into(),
        name: "latest".into(),
        tag: Some("unreleased".into()),
    }
}

fn frozen() -> Version {
    Version {
        dir: "9.9.9".into(),
        name: "9.9.9".into(),
        tag: None,
    }
}

/// The test manual's contents and pages.
fn fixture() -> (toc::Toc, BTreeMap<String, markup::Page>) {
    let dir = common::repo().join(FIXTURE);
    let toc = toc::parse(&std::fs::read_to_string(dir.join("toc.txt")).unwrap()).unwrap();
    let mut pages = BTreeMap::new();
    for name in ["index.md", "a.md", "b.md", "c.md"] {
        let text = std::fs::read_to_string(dir.join(name)).unwrap();
        pages.insert(name.to_string(), markup::parse(name, &text).unwrap());
    }
    (toc, pages)
}

/// The fixture's page `path` rendered for `v`.
fn render(path: &str, v: &Version) -> String {
    let (toc, pages) = fixture();
    let mut results = HashMap::new();
    results.insert(
        "a.md:1".to_string(),
        Outcome {
            output: "<b>&\n".into(),
            code: 0,
        },
    );
    let catalog = site::read_catalog(
        &std::fs::read_to_string(common::repo().join("crates/fung-cli/tests/data/spec/errors.txt"))
            .unwrap(),
    );
    let ctx = Ctx {
        toc: &toc,
        pages: &pages,
        results: &results,
        version: v,
        wordmark: "<svg class=\"wordmark\"></svg>",
        catalog: &catalog,
    };
    page_html(&ctx, path)
}

/// The text of the element that starts with `open` (up to its `</tag>`).
fn element<'a>(html: &'a str, open: &str, close: &str) -> &'a str {
    let i = html
        .find(open)
        .unwrap_or_else(|| panic!("no {open} in\n{html}"));
    let rest = &html[i..];
    &rest[..rest.find(close).unwrap() + close.len()]
}

#[test]
fn toc_parses_and_rejects() {
    let (toc, _) = fixture();
    assert_eq!(toc.parts.len(), 2);
    let learn = &toc.parts[0];
    assert_eq!(
        (learn.key.as_str(), learn.glyph.as_str(), learn.numbered),
        ("learn", "L", true)
    );
    assert_eq!(
        (
            learn.title.as_str(),
            learn.blurb.as_str(),
            learn.who.as_deref()
        ),
        ("배우기", "순서대로 읽는 책", Some("처음 온 사람"))
    );
    assert_eq!(
        learn.entries,
        vec![
            Entry::Page("a.md".into()),
            Entry::Group("둘째 묶음".into()),
            Entry::Page("b.md".into())
        ]
    );
    assert!(!toc.parts[1].numbered && toc.parts[1].who.is_none());
    for bad in [
        "page a.md\n",
        "part x X 제목\n",
        "part x X 제목 | 설명\npage a.md\npage a.md\n",
        "part x X 제목 | 설명\nchapter a.md\n",
        "part x X 제목 | 설명\npart x Y 다른 | 설명\n",
    ] {
        assert!(toc::parse(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn latest_has_banner_and_tag_frozen_does_not() {
    let html = render("a.md", &latest());
    assert!(html.contains("class=\"banner\""));
    assert!(html.contains(">unreleased<"));
    assert!(html.contains("data-current=\"latest\""));
    let html = render("a.md", &frozen());
    assert!(!html.contains("class=\"banner\""));
    assert!(!html.contains("unreleased"));
    assert!(html.contains("data-current=\"9.9.9\""));
}

#[test]
fn sidebar_marks_the_current_page() {
    let html = render("b.md", &latest());
    let side = element(&html, "<aside", "</aside>");
    assert!(
        side.contains("aria-current=\"page\">2. 둘째 장</a>"),
        "{side}"
    );
    assert!(side.contains("<li class=\"sub\">둘째 묶음</li>"), "{side}");
    let toc2 = element(side, "<ol class=\"toc2\">", "</ol>");
    assert!(toc2.contains("href=\"#two\"") && toc2.contains("href=\"#three\""));
    // only the current part lists its chapters
    assert_eq!(side.matches("<ol class=\"chapters\">").count(), 1);
    assert!(!side.contains("c.html\" aria-current"));
    assert!(side.contains("href=\"c.html\""));
}

#[test]
fn crumbs_and_pager() {
    let html = render("a.md", &latest());
    let crumbs = element(&html, "<nav class=\"crumbs\"", "</nav>");
    assert!(crumbs.contains("class=\"crumb-part\"") && crumbs.contains("<span>1장</span>"));
    let pager = element(&html, "<nav class=\"pager\"", "</nav>");
    assert!(pager.contains("다음 장") && pager.contains("href=\"b.html\""));
    assert!(!pager.contains("class=\"prev\""));
    let html = render("b.md", &latest());
    let crumbs = element(&html, "<nav class=\"crumbs\"", "</nav>");
    assert!(crumbs.contains("둘째 묶음") && crumbs.contains("<span>2장</span>"));
    assert!(!render("c.md", &latest()).contains("class=\"pager\""));
}

#[test]
fn cover_lists_the_parts() {
    let html = render("index.md", &latest());
    let cover = element(&html, "<div class=\"cover\">", "\n</div>");
    assert!(cover.contains("href=\"a.html\"") && cover.contains("href=\"c.html\""));
    assert!(cover.contains("처음 온 사람"));
    assert!(html.contains("<p class=\"lede\">시험용 표지.</p>"));
    assert!(html.contains("og:url"));
    assert!(!html.contains("class=\"crumbs\""));
    // the body's class must not be one the page's parts use
    assert!(html.contains("<body class=\"front\">"));
}

#[test]
fn entries_render_with_signature_and_anchor() {
    let html = render("c.md", &latest());
    let entry = element(
        &html,
        "<section class=\"entry\" id=\"fn-pick\">",
        "</section>",
    );
    assert!(
        entry.contains("<h3>pick<span class=\"by\">fung으로 작성</span></h3>"),
        "{entry}"
    );
    let summary = entry.find("<p>목록에서 하나를 고른다.</p>").unwrap();
    let sig = entry
        .find("<div class=\"sig\"><code>pick(xs, i = 0)</code></div>")
        .unwrap();
    assert!(summary < sig && entry.contains("자세한 설명."));
}

#[test]
fn error_entries_show_the_catalog() {
    let html = render("c.md", &latest());
    let entry = element(
        &html,
        "<section class=\"entry error\" id=\"E0606\">",
        "</section>",
    );
    assert!(
        entry.contains("discarded Err: %s") && entry.contains("handle it with"),
        "{entry}"
    );
    assert!(entry.contains("<code>Err</code>"));
    // the catalog's index links every entry
    let index = element(&html, "<div class=\"error-index\">", "</div>");
    assert!(index.contains("href=\"#E0606\""), "{index}");
}

#[test]
fn boxes_render() {
    let html = render("c.md", &latest());
    assert!(
        html.contains("<div class=\"box tip\"><b>권장</b>"),
        "{html}"
    );
}

#[test]
fn functions_list_links_every_entry() {
    let html = render("c.md", &latest());
    let list = element(&html, "<div class=\"fn-list\">", "\n</div>");
    assert!(list.contains("href=\"c.html#fn-pick\""), "{list}");
    assert!(list.contains("목록에서 하나를 고른다."));
    assert!(html.contains("id=\"fn-search\""));
}

#[test]
fn outputs_are_escaped() {
    let html = render("a.md", &latest());
    assert!(html.contains("&lt;b&gt;&amp;"), "{html}");
}

#[test]
fn pages_link_their_assets_relatively() {
    let html = render("a.md", &latest());
    assert!(html.contains("href=\"assets/style.css\""));
    assert!(html.contains("src=\"../versions.js\""));
    assert!(html.contains("src=\"assets/manual.js\""));
}

// ---- the site ----

/// A scratch directory for one test, empty.
fn scratch(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("fung-manual-build-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A copy of the test manual to change, at `<scratch>/manual` beside a
/// copy of the test catalog (`<scratch>/spec/errors.txt`).
fn manual_copy(tag: &str) -> std::path::PathBuf {
    let src = common::repo().join(FIXTURE);
    let root = scratch(tag);
    std::fs::create_dir_all(root.join("spec")).unwrap();
    std::fs::copy(
        common::repo().join("crates/fung-cli/tests/data/spec/errors.txt"),
        root.join("spec/errors.txt"),
    )
    .unwrap();
    let dst = root.join("manual");
    std::fs::create_dir_all(&dst).unwrap();
    for e in std::fs::read_dir(&src).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            std::fs::create_dir_all(dst.join(p.file_name().unwrap())).unwrap();
            for f in std::fs::read_dir(&p).unwrap() {
                let f = f.unwrap().path();
                std::fs::copy(
                    &f,
                    dst.join(p.file_name().unwrap())
                        .join(f.file_name().unwrap()),
                )
                .unwrap();
            }
        } else {
            std::fs::copy(&p, dst.join(p.file_name().unwrap())).unwrap();
        }
    }
    dst
}

#[test]
fn the_test_manual_builds() {
    let built = site::build(
        &common::repo().join(FIXTURE),
        &common::fung_bin(),
        &latest(),
    )
    .unwrap();
    let names: Vec<&str> = built.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        vec![
            "a.html",
            "assets/manual.js",
            "assets/style.css",
            "b.html",
            "c.html",
            "index.html"
        ]
    );
    let a = String::from_utf8(built["a.html"].clone()).unwrap();
    assert!(a.contains("&lt;b&gt;&amp;"), "the example ran: {a}");
}

#[test]
fn broken_links_are_errors() {
    let m = manual_copy("links");
    let a = m.join("a.md");
    let mut text = std::fs::read_to_string(&a).unwrap();
    text.push_str("\n[x](nope.html) [y](b.html#nope) [z](/abs.html) [w](b.html#three) [v](#one)\n");
    std::fs::write(&a, text).unwrap();
    let e = site::build(&m, &common::fung_bin(), &latest()).unwrap_err();
    assert!(
        e.contains("nope.html") && e.contains("b.html#nope") && e.contains("/abs.html"),
        "{e}"
    );
    assert!(!e.contains("#three") && !e.contains("#one"), "{e}");
    let _ = std::fs::remove_dir_all(m.parent().unwrap());
}

#[test]
fn duplicate_ids_are_errors() {
    let m = manual_copy("ids");
    let a = m.join("a.md");
    let mut text = std::fs::read_to_string(&a).unwrap();
    // a raw id that repeats a heading's; a data-id is not an id
    text.push_str("\n<p id=\"one\" data-id=\"two\">x</p>\n");
    std::fs::write(&a, text).unwrap();
    let e = site::build(&m, &common::fung_bin(), &latest()).unwrap_err();
    assert!(e.contains("a.html") && e.contains("`one`"), "{e}");
    assert!(!e.contains("`two`"), "{e}");
    let _ = std::fs::remove_dir_all(m.parent().unwrap());
}

#[test]
fn every_real_page_has_a_description() {
    let manual = common::repo().join("manual");
    let toc = toc::parse(&std::fs::read_to_string(manual.join("toc.txt")).unwrap()).unwrap();
    let mut pages = vec!["index.md".to_string()];
    for part in &toc.parts {
        pages.extend(part.pages().map(String::from));
    }
    for p in pages {
        let page = markup::parse(&p, &std::fs::read_to_string(manual.join(&p)).unwrap()).unwrap();
        assert!(page.description.is_some(), "{p} has no `description:`");
    }
}

#[test]
fn an_error_entry_shows_its_own_error() {
    let m = manual_copy("own-error");
    let c = m.join("c.md");
    let text = std::fs::read_to_string(&c).unwrap().replace(
        "버려진 `Err`.\n",
        "버려진 `Err`.\n\n```fung run exit=2 error=E0204\nprint(nope)\n```\n",
    );
    std::fs::write(&c, &text).unwrap();
    let e = site::build(&m, &common::fung_bin(), &latest()).unwrap_err();
    assert!(e.contains("E0606") && e.contains("E0204"), "{e}");
    let text = text.replace(
        "```fung run exit=2 error=E0204\nprint(nope)\n```",
        "```fung run exit=1 error=E0606\nparse_int(\"x\")\n```",
    );
    std::fs::write(&c, &text).unwrap();
    assert!(site::build(&m, &common::fung_bin(), &latest()).is_ok());
    let _ = std::fs::remove_dir_all(m.parent().unwrap());
}

#[test]
fn the_error_catalog_must_be_complete() {
    let m = manual_copy("catalog");
    let spec = m.parent().unwrap().join("spec/errors.txt");
    let mut cat = std::fs::read_to_string(&spec).unwrap();
    cat.push_str("E0101\tunexpected character %q\t\t\n");
    std::fs::write(&spec, cat).unwrap();
    let e = site::build(&m, &common::fung_bin(), &latest()).unwrap_err();
    assert!(e.contains("E0101") && e.contains(":::error"), "{e}");
    // an entry for a code the catalog does not have
    std::fs::write(&spec, "E0101\tx\t\t\n").unwrap();
    let c = m.join("c.md");
    let text = std::fs::read_to_string(&c)
        .unwrap()
        .replace(":::error E0606", ":::error E0101");
    std::fs::write(&c, text).unwrap();
    assert!(site::build(&m, &common::fung_bin(), &latest()).is_ok());
    std::fs::write(&spec, "E0606\tx\t\t\n").unwrap();
    let e = site::build(&m, &common::fung_bin(), &latest()).unwrap_err();
    assert!(e.contains("E0101"), "{e}");
    let _ = std::fs::remove_dir_all(m.parent().unwrap());
}

#[test]
fn a_manuscript_outside_the_contents_is_an_error() {
    let m = manual_copy("stray-md");
    std::fs::write(m.join("d.md"), "# 넷째\n").unwrap();
    let e = site::build(&m, &common::fung_bin(), &latest()).unwrap_err();
    assert!(e.contains("d.md") && e.contains("toc.txt"), "{e}");
    let _ = std::fs::remove_dir_all(m.parent().unwrap());
}

#[test]
fn versions_round_trip() {
    let vs = vec![latest(), frozen()];
    let js = site::versions_js(&vs);
    assert!(js.starts_with("window.FUNG_VERSIONS = ["), "{js}");
    assert_eq!(site::read_versions(&js), vs);
}

#[test]
fn old_addresses_forward_with_the_anchor() {
    let mut built = std::collections::BTreeMap::new();
    built.insert(
        "reference/prelude/collections.html".to_string(),
        b"<section class=\"entry\" id=\"fn-map\">".to_vec(),
    );
    built.insert("reference/modules/fs.html".to_string(), b"x".to_vec());
    built.insert(
        "reference/language.html".to_string(),
        b" id=\"scope\"".to_vec(),
    );
    let anchors = site::builtins_anchors(&built);
    assert_eq!(
        anchors.get("fn-map").map(String::as_str),
        Some("latest/reference/prelude/collections.html#fn-map")
    );
    assert_eq!(
        anchors.get("fs").map(String::as_str),
        Some("latest/reference/modules/fs.html")
    );
    assert!(!anchors.contains_key("scope"));
    let shared = site::shared(&[latest()], Some("설명"), &anchors);
    let names: Vec<&str> = shared.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        vec![
            "builtins.html",
            "index.html",
            "internals.html",
            "syntax.html",
            "versions.js"
        ]
    );
    let b = String::from_utf8(shared["builtins.html"].clone()).unwrap();
    assert!(
        b.contains("\"fn-map\": \"latest/reference/prelude/collections.html#fn-map\"")
            && b.contains(
                "location.replace(m[h] || \"latest/reference/functions.html\" + location.hash)"
            ),
        "{b}"
    );
    assert!(
        b.contains("http-equiv=\"refresh\" content=\"0; url=latest/reference/functions.html\"")
    );
    let i = String::from_utf8(shared["index.html"].clone()).unwrap();
    assert!(i.contains("latest/index.html") && i.contains("og:url"));
    assert!(i.contains("og:description\" content=\"설명\""));
}

#[test]
fn stray_files_are_reported() {
    let docs = scratch("sync");
    let built = site::build(
        &common::repo().join(FIXTURE),
        &common::fung_bin(),
        &latest(),
    )
    .unwrap();
    let shared = site::shared(&[latest()], None, &Default::default());
    let diff = site::sync(&docs, &built, &shared, false);
    assert!(diff.iter().any(|d| d.contains("a.html")), "{diff:?}");
    site::sync(&docs, &built, &shared, true);
    std::fs::write(docs.join("latest/old.html"), "x").unwrap();
    std::fs::create_dir_all(docs.join("0.4.0")).unwrap();
    std::fs::write(docs.join("0.4.0/kept.html"), "x").unwrap();
    let diff = site::sync(&docs, &built, &shared, false);
    assert_eq!(diff.len(), 1, "{diff:?}");
    assert!(diff[0].contains("old.html"));
    site::sync(&docs, &built, &shared, true);
    assert!(!docs.join("latest/old.html").exists());
    assert!(
        docs.join("0.4.0/kept.html").exists(),
        "frozen versions are left alone"
    );
    assert!(site::sync(&docs, &built, &shared, false).is_empty());
    let _ = std::fs::remove_dir_all(&docs);
}

#[test]
fn freeze_writes_a_new_version_only_once() {
    let docs = scratch("freeze");
    let manual = common::repo().join(FIXTURE);
    site::freeze(&docs, &manual, &common::fung_bin(), "9.9.9").unwrap();
    let a = std::fs::read_to_string(docs.join("9.9.9/a.html")).unwrap();
    assert!(!a.contains("class=\"banner\"") && a.contains("data-current=\"9.9.9\""));
    let vs = site::read_versions(&std::fs::read_to_string(docs.join("versions.js")).unwrap());
    assert_eq!(vs, vec![latest(), frozen()]);
    assert!(site::freeze(&docs, &manual, &common::fung_bin(), "9.9.9").is_err());
    let _ = std::fs::remove_dir_all(&docs);
}

// ---- the real manual ----

/// `docs/latest/`, `docs/versions.js` and the old addresses are what
/// `manual/` builds. `FUNG_UPDATE_MANUAL=1` writes them instead (review
/// the diff before committing). Frozen versions are never touched.
#[test]
fn the_manual_is_up_to_date() {
    let repo = common::repo();
    let manual = repo.join("manual");
    let docs = repo.join("docs");
    let built = site::build(&manual, &common::fung_bin(), &site::latest())
        .unwrap_or_else(|e| panic!("the manual does not build:\n{e}"));
    let versions = match std::fs::read_to_string(docs.join("versions.js")) {
        Ok(js) => site::read_versions(&js),
        Err(_) => vec![site::latest()],
    };
    let cover = markup::parse(
        "index.md",
        &std::fs::read_to_string(manual.join("index.md")).unwrap(),
    )
    .unwrap();
    let shared = site::shared(
        &versions,
        cover.description.as_deref(),
        &site::builtins_anchors(&built),
    );
    let update = std::env::var_os("FUNG_UPDATE_MANUAL").is_some();
    let diff = site::sync(&docs, &built, &shared, update);
    assert!(
        update || diff.is_empty(),
        "docs/ is not what manual/ builds:\n{}\n\nregenerate with: FUNG_UPDATE_MANUAL=1 cargo test -p fung-cli --test manual_build",
        diff.join("\n")
    );
}

/// `FUNG_MANUAL_FREEZE=x.y.z cargo test -p fung-cli --test manual_build
/// freeze_a_release` freezes the manual as release x.y.z (once). Without
/// the variable it does nothing.
#[test]
fn freeze_a_release() {
    let Some(name) = std::env::var_os("FUNG_MANUAL_FREEZE") else {
        return;
    };
    let repo = common::repo();
    site::freeze(
        &repo.join("docs"),
        &repo.join("manual"),
        &common::fung_bin(),
        &name.to_string_lossy(),
    )
    .unwrap();
}

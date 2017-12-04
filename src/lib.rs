#[macro_use]
extern crate nom;
use std::str;

use nom::{line_ending, digit, space};

named!(
    rest_of_line<&str>,
    do_parse!(
        content: map_res!(
            nom::not_line_ending,
            str::from_utf8
        ) >>
        line_ending >>
        (content)
    )
);

named!(
    compiling<()>,
    do_parse!(
      ws!(tag!("Compiling")) >>
      rest_of_line >>
      ()
    )
);

named!(
    downloading<()>,
    do_parse!(
      ws!(tag!("Downloading")) >>
      rest_of_line >>
      ()
    )
);

named!(
  installing<()>,
    do_parse!(
      ws!(tag!("Installing")) >>
      rest_of_line >>
      ()
    )
);

named!(
    updating<()>,
      do_parse!(
        ws!(tag!("Updating")) >>
        rest_of_line >>
        ()
      )
);

named!(
    finished<()>,
    do_parse!(
        ws!(tag!("Finished")) >>
        rest_of_line >>
        ()
    )
);

named!(
    suite_line<&str>,
    do_parse!(
        ws!(
            alt!(tag!("Running") | tag!("Doc-tests"))
        ) >>
        name: rest_of_line >>
        (name)
    )
);

named!(
    suite_count<()>,
    do_parse!(
        ws!(tag!("running")) >>
        rest_of_line >>
        ()
    )
);

named!(
    ok<&str>,
    map!(tag!("ok"),
    |_| "pass")
);

named!(
    failed<&str>,
    map!(tag!("FAILED"),
    |_| "fail")
);

named!(
    ok_or_failed<&str>,
    alt!(ok | failed)
);

#[derive(Debug, PartialEq)]
pub struct Test<'a, 'b, 'c> {
    pub name: &'a str,
    pub status: &'b str,
    pub error: Option<&'c str>,
}

named!(
    test_result<Test>,
    do_parse!(
        tag!("test") >>
        space >>
        name: map_res!(
            take_until_s!(" ..."),
            str::from_utf8
        ) >>
        tag!(" ...") >>
        status: ws!(ok_or_failed) >>
        (Test {
            name: name,
            status: status,
            error: None
        })
    )
);

named!(
    test_results<Vec<Test> >,
    many0!(
        test_result
    )
);

named!(
    digits<i64>,
    map_res!(
        map_res!(
            ws!(digit),
            str::from_utf8
        ),
        str::FromStr::from_str
    )
);

#[derive(Debug, PartialEq)]
pub struct SuiteResult<'a> {
    pub state: &'a str,
    pub passed: i64,
    pub failed: i64,
    pub ignored: i64,
    pub total: i64,
    pub measured: i64,
}

named!(
    suite_result<SuiteResult>,
    do_parse!(
        ws!(tag!("test result: ")) >>
        state: ok_or_failed >>
        char!('.') >>
        passed: digits >>
        tag!("passed;") >>
        failed: digits >>
        tag!("failed;") >>
        ignored: digits >>
        tag!("ignored;") >>
        measured: digits >>
        tag!("measured;") >>
        digits >>
        ws!(tag!("filtered out")) >>
        (SuiteResult {
          state:state,
          passed:passed,
          failed:failed,
          ignored:ignored,
          total: passed + failed + ignored,
          measured:measured
        })
    )
);

named!(
    fail_line<&str>,
    do_parse!(
        ws!(tag!("----")) >>
        name: map_res!(
            take_until!(" "),
            str::from_utf8
        ) >>
        ws!(tag!("stdout")) >>
        ws!(tag!("----")) >>
        (name)
    )
);

#[derive(Debug, PartialEq)]
pub struct Failure<'a, 'b> {
    pub name: &'a str,
    pub error: &'b str,
}

named!(
    failure<Failure>,
    do_parse!(
        name: fail_line >>
        error: rest_of_line >>
        opt!(
            tag!("note: Run with `RUST_BACKTRACE=1` for a backtrace.")
        ) >>
        line_ending >>
        line_ending >>
        (Failure {
            name:name,
            error:error
        })
    )
);

named!(failures<Vec<Failure> >, many1!(failure));

named!(fail_opt<Option<Vec<Failure> > >,
    opt!(
        do_parse!(
            ws!(
                tag!("failures:")
            ) >>
            f: failures >>
            take_until!(
                "test result: "
            ) >>
            (f)
        )
    )
);

#[derive(Debug, PartialEq)]
pub struct Suite<'a, 'b, 'c, 'd, 'e> {
    pub name: &'a str,
    pub state: &'b str,
    pub passed: i64,
    pub failed: i64,
    pub ignored: i64,
    pub measured: i64,
    pub total: i64,
    pub tests: Vec<Test<'c, 'd, 'e>>,
}

fn find_message_by_name<'a, 'b>(name: &str, failures: &Vec<Failure<'a, 'b>>) -> Option<&'b str> {
    failures.iter().find(|x| x.name == name).map(|x| x.error)
}

fn handle_parsed_suite<'a, 'b, 'c, 'd, 'e>(
    name: &'a str,
    tests: Vec<Test<'c, 'd, 'e>>,
    failures: Option<Vec<Failure<'e, 'e>>>,
    result: SuiteResult<'b>,
) -> Suite<'a, 'b, 'c, 'd, 'e> {
    let tests_with_failures = match failures {
        Some(xs) => {
            tests
                .iter()
                .map(|t| {
                    Test {
                        error: find_message_by_name(t.name, &xs),
                        name: t.name,
                        status: t.status,
                    }
                })
                .collect()
        }
        None => tests,
    };

    Suite {
        name: name,
        tests: tests_with_failures,
        state: result.state,
        total: result.total,
        passed: result.passed,
        failed: result.failed,
        ignored: result.ignored,
        measured: result.measured,
    }
}

named!(
    suite_parser<Suite>,
    do_parse!(
        name: suite_line >>
        suite_count >>
        tests: test_results >>
        failures: fail_opt >>
        result: suite_result >>
        (handle_parsed_suite(name, tests, failures, result))
    )
);

named!(
    suites_parser<Vec<Suite > >,
    many1!(suite_parser)
);

named!(
  compile_error<Vec<Suite > >,
  do_parse!(
    ws!(tag!("error")) >>
    opt_res!(
      do_parse!(
        char!('[') >>
        take_until_and_consume!("]") >>
        ()
      )
    ) >>
    ws!(char!(':')) >>
    error: map_res!(
            take_till!(|c| c == 0x0),
            str::from_utf8
        ) >>
    (vec![Suite {
        name: "unknown",
        state: "fail",
        total: 1,
        passed: 0,
        failed: 1,
        ignored: 0,
        measured: 0,
        tests: vec![
          Test {
            name: "compile failed",
            status: "fail",
            error: Some(error.into())
          }
        ]
    }])
  )
);

named!(
    pub cargo_test_result_parser<Vec<Suite > >,
    do_parse!(
        many0!(
          alt!(updating | downloading | installing | compiling | finished)
        ) >>
        suites: alt!(suites_parser | compile_error) >>
        (suites)
    )
);


#[cfg(test)]
mod parser_tests {
    use nom::IResult;
    use std::fmt::Debug;
    use super::{downloading, compiling, installing, finished, suite_line, suite_count,
                ok_or_failed, Test, test_result, test_results, digits, suite_result, SuiteResult,
                cargo_test_result_parser, Suite, fail_line, failure, Failure, failures};

    fn assert_done<R: PartialEq + Debug>(l: IResult<&[u8], R>, r: R) {
        assert_eq!(
          l,
          IResult::Done(&b""[..], r)
      )
    }

    #[test]
    fn it_should_parse_a_downloading_line() {
        let output = &b" Downloading nvpair-sys v0.1.0
"[..];

        assert_done(downloading(output), ())
    }

    #[test]
    fn it_should_parse_an_installing_line() {
        let output = &b" Installing cargo-test-junit v0.6.2
"[..];

        assert_done(installing(output), ())
    }

    #[test]
    fn it_should_match_a_compiler_line() {
        let output = &b"   Compiling docker-command v0.1.0 (file:///Users/joegrund/projects/docker-command-rs)
"
            [..];

        assert_done(compiling(output), ());
    }

    #[test]
    fn it_should_parse_finish_line() {
        let result = finished(
            &b"    Finished debug [unoptimized + debuginfo] target(s) in 0.0 secs
"[..],
        );

        assert_done(result, ());
    }

    #[test]
    fn it_should_parse_suite_line() {
        let result = suite_line(
            &b"Running target/debug/deps/docker_command-be014e20fbd07382
"[..],
        );

        assert_done(result, "target/debug/deps/docker_command-be014e20fbd07382");
    }

    #[test]
    fn it_should_parse_suite_count() {
        let result = suite_count(
            &b"running 0 tests
"[..],
        );

        assert_done(result, ());
    }

    #[test]
    fn it_should_match_ok() {
        assert_done(ok_or_failed(&b"ok"[..]), "pass");
    }

    #[test]
    fn it_should_match_failed() {
        assert_done(ok_or_failed(&b"FAILED"[..]), "fail");
    }

    #[test]
    fn it_should_parse_test_result() {
        let result = test_result(&b"test it_runs_a_command ... ok"[..]);

        assert_done(
            result,
            Test {
                name: "it_runs_a_command",
                status: "pass",
                error: None,
            },
        );
    }

    #[test]
    fn it_should_parse_test_results() {
        let result = test_results(
            &b"test tests::it_should_parse_first_line ... ok
test tests::it_should_parse_a_status_line ... ok
test tests::it_should_parse_test_output ... ok
test tests::it_should_parse_suite_line ... FAILED
"
                [..],
        );

        assert_done(
            result,

            vec![
                Test {
                    name: "tests::it_should_parse_first_line",
                    status: "pass",
                    error: None
                },
                Test {
                    name: "tests::it_should_parse_a_status_line",
                    status: "pass",
                    error: None
                },
                Test {
                    name: "tests::it_should_parse_test_output",
                    status: "pass",
                    error: None
                },
                Test {
                    name: "tests::it_should_parse_suite_line",
                    status: "fail",
                    error: None
                }
              ],
        );
    }

    #[test]
    fn it_should_capture_digits() {
        assert_done(digits(b"10"), 10);
    }

    #[test]
    fn it_should_parse_a_suite_result() {
        let result = suite_result(
            &b"test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out"[..],
        );

        assert_done(
            result,
            SuiteResult {
                state: "fail",
                passed: 3,
                failed: 1,
                ignored: 0,
                total: 4,
                measured: 0,
            },
        );
    }

    #[test]
    fn it_should_parse_successful_test_output() {
        let output = &b"    Finished debug [unoptimized + debuginfo] target(s) in 0.0 secs
       Running target/debug/cargo_test_junit-83252957c74e106d

running 2 tests
test tests::it_should_match_failed ... ok
test tests::it_should_parse_first_line ... ok


  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  "
            [..];

        let result = cargo_test_result_parser(output);

        assert_done(
            result,
            vec![Suite {
            name: "target/debug/cargo_test_junit-83252957c74e106d",
            state: "pass",
            tests: vec![
                Test {
                    name: "tests::it_should_match_failed",
                    status: "pass",
                    error: None
                },
                Test {
                    name: "tests::it_should_parse_first_line",
                    status: "pass",
                    error: None
                }
            ],
            passed: 2,
            failed: 0,
            ignored: 0,
            measured: 0,
            total: 2
        }],
        );
    }

    #[test]
    fn test_fail_line() {
        let output = b"---- fail stdout ----";

        assert_done(fail_line(output), "fail");
    }

    #[test]
    fn test_failure() {
        let output = b"---- fail stdout ----
  thread 'fail' panicked at 'assertion failed: `(left == right)` (left: `1`, right: `2`)', tests/integration_test.rs:16
note: Run with `RUST_BACKTRACE=1` for a backtrace.

";
        assert_done(
            failure(output),
            Failure {
                name: "fail",
                error: "thread 'fail' panicked at 'assertion failed: `(left == right)` \
                                (left: `1`, right: `2`)', tests/integration_test.rs:16",
            },
        );
    }

    #[test]
    fn test_failures() {
        let output = b"---- fail stdout ----
          thread 'fail' panicked at 'assertion failed: `(left == right)` (left: `1`, right: `2`)', tests/integration_test.rs:16
note: Run with `RUST_BACKTRACE=1` for a backtrace.

        ---- fail2 stdout ----
          thread 'fail2' panicked at 'assertion failed: `(left == right)` (left: `3`, right: `2`)', tests/integration_test.rs:22


";

        assert_done(
            failures(output),
            vec![
                Failure {
                    name: "fail",
                    error: "thread 'fail' panicked at 'assertion failed: `(left == right)` (left: `1`, right: `2`)', tests/integration_test.rs:16"
                },
                Failure {
                    name: "fail2",
                    error: "thread 'fail2' panicked at 'assertion failed: `(left == right)` (left: `3`, right: `2`)', tests/integration_test.rs:22"
                }
            ],
        );
    }

    #[test]
    fn test_fail_run() {
        let output = b"  Compiling blah v0.1.0 (file:blah)
        Finished debug [unoptimized + debuginfo] target(s) in 0.32 secs
        Running target/debug/deps/docker_command-be014e20fbd07382

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

        Running target/debug/integration_test-d4fc68dd5824cbb9

running 3 tests
test fail ... FAILED
test fail2 ... FAILED
test it_runs_a_command ... ok

failures:

---- fail stdout ----
thread 'fail' panicked at 'assertion failed: `(left == right)` (left: `1`, right: `2`)', tests/integration_test.rs:16
note: Run with `RUST_BACKTRACE=1` for a backtrace.

---- fail2 stdout ----
thread 'fail2' panicked at 'assertion failed: `(left == right)` (left: `3`, right: `2`)', tests/integration_test.rs:22


failures:
        fail
        fail2

test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out

error: test failed";

        let x = match cargo_test_result_parser(output) {
            IResult::Done(_, x) => x,
            _ => panic!("BOOM!"),
        };

        assert_eq!(
            x,
            vec![
                Suite {
                    name: "target/debug/deps/docker_command-be014e20fbd07382",
                    state: "pass",
                    passed: 0,
                    failed: 0,
                    ignored: 0,
                    measured: 0,
                    total: 0,
                    tests: vec![]
                },
                Suite {
                    name: "target/debug/integration_test-d4fc68dd5824cbb9",
                    state: "fail",
                    passed: 1,
                    failed: 2,
                    ignored: 0,
                    measured: 0,
                    total: 3,
                    tests: vec![
                        Test {
                            name: "fail",
                            status: "fail",
                            error: Some("thread \'fail\' panicked at \'assertion failed: `(left == right)` (left: `1`, right: `2`)\', tests/integration_test.rs:16")
                        },
                        Test {
                            name: "fail2",
                            status: "fail",
                            error: Some("thread \'fail2\' panicked at \'assertion failed: `(left == right)` (left: `3`, right: `2`)\', tests/integration_test.rs:22")
                        },
                        Test {
                            name: "it_runs_a_command",
                            status: "pass",
                            error: None
                        }
                    ]
                }
            ]
        );
    }

    #[test]
    fn test_success_run() {
        let output = b"   Compiling rustc-serialize v0.3.22
   Compiling toml v0.2.1
   Compiling pre-commit v0.5.2
   Compiling foo v0.1.0 (file:///foo)
    Finished debug [unoptimized + debuginfo] target(s) in 12.11 secs
     Running target/debug/deps/foo-5a7be5d1b9c8e0f6

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running target/debug/integration_test-283604d1063344ba

running 1 test
test it_runs_a_command ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests foo

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out";

        assert_done(
            cargo_test_result_parser(output),
            vec![
                      Suite {
                          name: "target/debug/deps/foo-5a7be5d1b9c8e0f6",
                          state: "pass",
                          passed: 0,
                          failed: 0,
                          ignored: 0,
                          measured: 0,
                          total: 0,
                          tests: vec![]
                      },
                      Suite {
                          name: "target/debug/integration_test-283604d1063344ba",
                          state: "pass",
                          passed: 1,
                          failed: 0,
                          ignored: 0,
                          measured: 0,
                          total: 1,
                          tests: vec![
                              Test {
                                  name: "it_runs_a_command",
                                  status: "pass",
                                  error: None
                              }
                          ]
                      },
                      Suite {
                          name: "foo",
                          state: "pass",
                          passed: 0,
                          failed: 0,
                          ignored: 0,
                          measured: 0,
                          total: 0,
                          tests: vec![]
                      }
                  ],
        );
    }

    #[test]
    fn test_full_run() {
        let output = b"    Updating registry `https://github.com/rust-lang/crates.io-index`
 Downloading nvpair-sys v0.1.0
 Downloading bindgen v0.30.0
 Downloading pkg-config v0.3.9
 Downloading clap v2.27.1
 Downloading which v1.0.3
 Downloading cfg-if v0.1.2
 Downloading lazy_static v0.2.10
 Downloading clang-sys v0.19.0
 Downloading log v0.3.8
 Downloading env_logger v0.4.3
 Downloading regex v0.2.2
 Downloading syntex_syntax v0.58.1
 Downloading aster v0.41.0
 Downloading quasi v0.32.0
 Downloading cexpr v0.2.2
 Downloading peeking_take_while v0.1.2
 Downloading textwrap v0.9.0
 Downloading unicode-width v0.1.4
 Downloading vec_map v0.8.0
 Downloading strsim v0.6.0
 Downloading atty v0.2.3
 Downloading bitflags v0.9.1
 Downloading ansi_term v0.9.0
 Downloading libc v0.2.33
 Downloading libloading v0.4.2
 Downloading glob v0.2.11
 Downloading aho-corasick v0.6.3
 Downloading utf8-ranges v1.0.0
 Downloading thread_local v0.3.4
 Downloading memchr v1.0.2
 Downloading regex-syntax v0.4.1
 Downloading unreachable v1.0.0
 Downloading void v1.0.2
 Downloading bitflags v0.8.2
 Downloading syntex_pos v0.58.1
 Downloading rustc-serialize v0.3.24
 Downloading unicode-xid v0.0.4
 Downloading syntex_errors v0.58.1
 Downloading term v0.4.6
 Downloading nom v3.2.1
 Downloading quasi_codegen v0.32.0
 Downloading syntex v0.58.1
   Compiling bitflags v0.9.1
   Compiling unicode-xid v0.0.4
   Compiling libc v0.2.33
   Compiling void v1.0.2
   Compiling ansi_term v0.9.0
   Compiling libloading v0.4.2
   Compiling utf8-ranges v1.0.0
   Compiling log v0.3.8
   Compiling lazy_static v0.2.10
   Compiling term v0.4.6
   Compiling unicode-width v0.1.4
   Compiling nvpair-sys v0.1.0
   Compiling pkg-config v0.3.9
   Compiling glob v0.2.11
   Compiling cfg-if v0.1.2
   Compiling regex-syntax v0.4.1
   Compiling peeking_take_while v0.1.2
   Compiling bitflags v0.8.2
   Compiling vec_map v0.8.0
   Compiling strsim v0.6.0
   Compiling rustc-serialize v0.3.24
   Compiling which v1.0.3
   Compiling atty v0.2.3
   Compiling memchr v1.0.2
   Compiling unreachable v1.0.0
   Compiling textwrap v0.9.0
   Compiling clang-sys v0.19.0
   Compiling syntex_pos v0.58.1
   Compiling nom v3.2.1
   Compiling aho-corasick v0.6.3
   Compiling thread_local v0.3.4
   Compiling clap v2.27.1
   Compiling syntex_errors v0.58.1
   Compiling cexpr v0.2.2
   Compiling regex v0.2.2
   Compiling syntex_syntax v0.58.1
   Compiling env_logger v0.4.3
   Compiling quasi v0.32.0
   Compiling syntex v0.58.1
   Compiling aster v0.41.0
   Compiling quasi_codegen v0.32.0
   Compiling bindgen v0.30.0
   Compiling libzfs-sys v0.1.0 (file:///vagrant/libzfs-sys)
    Finished dev [unoptimized + debuginfo] target(s) in 862.1 secs
     Running target/debug/deps/libzfs_sys-a797c24cd4b4a7ea

running 3 tests
test bindgen_test_layout_zpool_handle ... ok
test tests::open_close_handle ... ok
test tests::pool_search_import_list_export ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests libzfs-sys

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

    ";

        assert_done(
            cargo_test_result_parser(output),
            vec![
                          Suite {
                              name: "target/debug/deps/libzfs_sys-a797c24cd4b4a7ea",
                              state: "pass",
                              passed: 3,
                              failed: 0,
                              ignored: 0,
                              measured: 0,
                              total: 3,
                              tests: vec![
                                Test {
                                  name: "bindgen_test_layout_zpool_handle",
                                  status: "pass",
                                  error: None
                                },
                                Test {
                                  name: "tests::open_close_handle",
                                  status: "pass",
                                  error: None
                                },
                                Test {
                                  name: "tests::pool_search_import_list_export",
                                  status: "pass",
                                  error: None
                                }
                              ]
                          },
                          Suite {
                              name: "libzfs-sys",
                              state: "pass",
                              passed: 0,
                              failed: 0,
                              ignored: 0,
                              measured: 0,
                              total: 0,
                              tests: vec![]
                          }
                      ],
        );

    }

    #[test]
    pub fn compile_fail() {
        let output = b"    Updating registry `https://github.com/rust-lang/crates.io-index`
 Downloading cargo-test-junit v0.6.2
  Installing cargo-test-junit v0.6.2
 Downloading test-to-vec v0.4.2
 Downloading nom v2.2.1
 Downloading clap v2.28.0
 Downloading sxd-document v0.2.4
 Downloading duct v0.4.0
 Downloading textwrap v0.9.0
 Downloading bitflags v1.0.1
 Downloading vec_map v0.8.0
 Downloading unicode-width v0.1.4
 Downloading strsim v0.6.0
 Downloading atty v0.2.3
 Downloading ansi_term v0.10.2
 Downloading peresil v0.3.0
 Downloading typed-arena v1.3.0
 Downloading libc v0.2.34
 Downloading crossbeam v0.3.0
   Compiling nom v2.2.1
   Compiling libc v0.2.34
   Compiling crossbeam v0.3.0
   Compiling unicode-width v0.1.4
   Compiling strsim v0.6.0
   Compiling bitflags v1.0.1
   Compiling vec_map v0.8.0
   Compiling peresil v0.3.0
   Compiling typed-arena v1.3.0
   Compiling ansi_term v0.10.2
   Compiling test-to-vec v0.4.2
   Compiling atty v0.2.3
   Compiling duct v0.4.0
   Compiling textwrap v0.9.0
   Compiling sxd-document v0.2.4
   Compiling clap v2.28.0
   Compiling cargo-test-junit v0.6.2
    Finished release [optimized] target(s) in 114.51 secs
  Installing /root/.cargo/bin/cargo-test-junit
    Updating git repository `https://github.com/jgrund/rust-libzfs.git`
 Downloading pkg-config v0.3.9
 Downloading bindgen v0.30.0
 Downloading env_logger v0.4.3
 Downloading peeking_take_while v0.1.2
 Downloading quasi v0.32.0
 Downloading cfg-if v0.1.2
 Downloading clap v2.27.1
 Downloading aster v0.41.0
 Downloading syntex_syntax v0.58.1
 Downloading regex v0.2.2
 Downloading lazy_static v0.2.11
 Downloading which v1.0.3
 Downloading clang-sys v0.19.0
 Downloading cexpr v0.2.2
 Downloading log v0.3.8
 Downloading memchr v1.0.2
 Downloading utf8-ranges v1.0.0
 Downloading thread_local v0.3.4
 Downloading aho-corasick v0.6.3
 Downloading regex-syntax v0.4.1
 Downloading libc v0.2.33
 Downloading unreachable v1.0.0
 Downloading void v1.0.2
 Downloading syntex_errors v0.58.1
 Downloading rustc-serialize v0.3.24
 Downloading bitflags v0.8.2
 Downloading syntex_pos v0.58.1
 Downloading unicode-xid v0.0.4
 Downloading term v0.4.6
 Downloading ansi_term v0.9.0
 Downloading bitflags v0.9.1
 Downloading libloading v0.4.2
 Downloading glob v0.2.11
 Downloading nom v3.2.1
 Downloading quasi_codegen v0.32.0
 Downloading syntex v0.58.1
 Downloading cstr-argument v0.0.2
   Compiling strsim v0.6.0
   Compiling unicode-xid v0.0.4
   Compiling glob v0.2.11
   Compiling log v0.3.8
   Compiling nvpair-sys v0.1.0 (https://github.com/jgrund/rust-libzfs.git?rev=get-values#470f3014)
   Compiling rustc-serialize v0.3.24
   Compiling vec_map v0.8.0
   Compiling cfg-if v0.1.2
   Compiling unicode-width v0.1.4
   Compiling libloading v0.4.2
   Compiling pkg-config v0.3.9
   Compiling lazy_static v0.2.11
   Compiling ansi_term v0.9.0
   Compiling peeking_take_while v0.1.2
   Compiling libc v0.2.33
   Compiling utf8-ranges v1.0.0
   Compiling term v0.4.6
   Compiling bitflags v0.8.2
   Compiling bitflags v0.9.1
   Compiling regex-syntax v0.4.1
   Compiling void v1.0.2
   Compiling clang-sys v0.19.0
   Compiling syntex_pos v0.58.1
   Compiling textwrap v0.9.0
   Compiling memchr v1.0.2
   Compiling atty v0.2.3
   Compiling which v1.0.3
   Compiling unreachable v1.0.0
   Compiling syntex_errors v0.58.1
   Compiling nom v3.2.1
   Compiling cstr-argument v0.0.2
   Compiling aho-corasick v0.6.3
   Compiling clap v2.27.1
   Compiling thread_local v0.3.4
   Compiling syntex_syntax v0.58.1
   Compiling cexpr v0.2.2
   Compiling nvpair v0.2.0 (https://github.com/jgrund/rust-libzfs.git?rev=get-values#470f3014)
   Compiling regex v0.2.2
   Compiling aster v0.41.0
   Compiling syntex v0.58.1
   Compiling quasi v0.32.0
   Compiling env_logger v0.4.3
   Compiling quasi_codegen v0.32.0
   Compiling bindgen v0.30.0
   Compiling libzfs-sys v0.1.0 (file:///vagrant/libzfs-sys)
   Compiling libzfs v0.1.0 (file:///vagrant/libzfs)
error[E0369]: binary operation `==` cannot be applied to type `std::result::Result<nvpair::NvData, std::io::Error>`
   --> libzfs/src/lib.rs:134:9
    |
134 |         assert_eq!(state, Ok(nvpair::NvData::Uint64(sys::pool_state::POOL_STATE_EXPORTED as u64)));
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: an implementation of `std::cmp::PartialEq` might be missing for `std::result::Result<nvpair::NvData, std::io::Error>`
    = note: this error originates in a macro outside of the current crate

error: aborting due to previous error

error: Could not compile `libzfs`.

To learn more, run the command again with --verbose.
";

        assert_done(
            cargo_test_result_parser(output),
            vec![
                  Suite {
                    name: "unknown",
                    state: "fail",
                    passed: 0,
                    failed: 1,
                    ignored: 0,
                    measured: 0,
                    total: 1,
                    tests: vec![
                        Test {
                            name: "compile failed",
                            status: "fail",
                            error: Some("binary operation `==` cannot be applied to type `std::result::Result<nvpair::NvData, std::io::Error>`
   --> libzfs/src/lib.rs:134:9
    |
134 |         assert_eq!(state, Ok(nvpair::NvData::Uint64(sys::pool_state::POOL_STATE_EXPORTED as u64)));
    |         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    |
    = note: an implementation of `std::cmp::PartialEq` might be missing for `std::result::Result<nvpair::NvData, std::io::Error>`
    = note: this error originates in a macro outside of the current crate

error: aborting due to previous error

error: Could not compile `libzfs`.

To learn more, run the command again with --verbose.
")
                        },
                    ]
                }
                      ],
        );
    }
}

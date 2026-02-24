#[cfg(feature = "std")]
mod bench_util;

#[cfg(feature = "std")]
mod main {
    use std::fs::File;
    use std::io::{BufReader, Read};
    use std::os::unix::fs::OpenOptionsExt;
    use std::time::Duration;

    use benchmarking::measure_function_with_times;
    use benchmarking::Measurer;
    use libc::O_DIRECT;
    use semisync_read::SemisyncReader;

    use quake_util::qmap::parse;

    use crate::bench_util::prepare_file;

    fn parse_file_semisync(file_path: &str) {
        let f = File::options()
            .read(true)
            //.custom_flags(O_DIRECT)
            .open(file_path)
            .unwrap();
        let mut reader = SemisyncReader::new(f).unwrap();
        let _ = parse(&mut reader).unwrap();
    }

    fn parse_file_buffered(file_path: &str) {
        let f = File::open(file_path).unwrap();
        let mut reader = BufReader::with_capacity(8192, f);
        let _ = parse(&mut reader).unwrap();
    }

    fn parse_file_slurp(file_path: &str) {
        let mut f = File::open(file_path).unwrap();
        let mut v = Vec::new();
        f.read_to_end(&mut v).unwrap();
        let mut slice = &v[..];
        let _ = parse(&mut slice).unwrap();
    }

    fn measure_read_parse(
        method: &'static dyn Fn(&str),
        path: &str,
    ) -> Duration {
        let path = String::from(path);

        let results = measure_function_with_times(1, move |measurer| {
            measurer.measure(|| {
                method(&path);
            });
        })
        .unwrap();

        results.elapsed()
    }

    fn measure_slurp(path: &str) -> (Vec<u8>, Duration) {
        let mut v = Vec::new();

        let results =
            measure_function_with_times(1, &mut |measurer: &mut Measurer| {
                measurer.measure(|| {
                    let mut f = File::open(path).unwrap();
                    f.read_to_end(&mut v).unwrap();
                });
            })
            .unwrap();

        (v, results.elapsed())
    }

    fn measure_just_parse(buf: &mut impl Read) -> Duration {
        let results = measure_function_with_times(1, |measurer| {
            measurer.measure(|| {
                let _ = parse(buf).unwrap();
            });
        })
        .unwrap();

        results.elapsed()
    }

    pub fn run_benches() {
        let map_names = ["ad_heresp2.map", "standard.map"];
        let maps = map_names
            .iter()
            .map(|&m| (m, prepare_file(m).unwrap()))
            .collect::<Vec<_>>();

        let methods: [(&str, &dyn Fn(&str)); _] = [
            ("asynchronous IO", &parse_file_semisync),
            ("buffered IO", &parse_file_buffered),
            ("slurping", &parse_file_slurp),
        ];

        for (map_name, map_path) in &maps {
            for (method_name, method) in methods {
                println!(
                    "Took {:?} to parse {map_name} with {method_name}",
                    measure_read_parse(method, &map_path),
                );
            }
        }

        let (heresp2_name, heresp2_path) = &maps[0];
        let (buf, slurp_duration) = measure_slurp(&heresp2_path);
        let mut slice = &buf[..];

        println!("Took {slurp_duration:?} to slurp {heresp2_name}");

        println!(
            "Took {:?} to JUST parse {heresp2_name}",
            measure_just_parse(&mut slice),
        );
    }
}

#[cfg(not(feature = "std"))]
mod main {
    pub fn run_benches() {}
}

fn main() {
    main::run_benches();
}

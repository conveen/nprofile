use std::io::Write;

pub fn configure_logging(debug: bool) {
    let mut logger_builder = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    if debug {
        logger_builder
            .format(|formatter, record| {
                writeln!(
                    formatter,
                    "{}\t{}\t{:?}\t{}",
                    formatter.timestamp_seconds(),
                    record.level(),
                    record.module_path(),
                    record.args()
                )
            })
            .filter_level(log::LevelFilter::Debug);
    } else {
        logger_builder
            .format(|formatter, record| {
                writeln!(formatter, "{}\t{}\t{}", formatter.timestamp_seconds(), record.level(), record.args())
            })
            .filter_level(log::LevelFilter::Info);
    }
    logger_builder.init();
}

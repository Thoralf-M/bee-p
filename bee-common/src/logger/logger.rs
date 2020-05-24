// Copyright 2020 IOTA Stiftung
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except in compliance with
// the License. You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on
// an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and limitations under the License.

use crate::LoggerConfig;

#[derive(Debug)]
#[non_exhaustive]
pub enum LoggerError {
    File,
    Apply,
}

pub fn logger_init(config: LoggerConfig) -> Result<(), LoggerError> {
    let mut logger = fern::Dispatch::new().format(|out, message, record| {
        out.finish(format_args!(
            "{}[{}][{}] {}",
            chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
            record.target(),
            record.level(),
            message
        ))
    });

    for output in config.outputs {
        let mut dispatcher = fern::Dispatch::new().level(output.level);

        dispatcher = if output.name == "stdout" {
            dispatcher.chain(std::io::stdout())
        } else {
            dispatcher.chain(fern::log_file(output.name).map_err(|_| LoggerError::File)?)
        };

        logger = logger.chain(dispatcher);
    }

    logger.apply().map_err(|_| LoggerError::Apply)?;

    Ok(())
}

// pub fn logger_init(config: LoggerConfig) -> Result<(), LoggerError> {
//     let conf = config.clone();
//
//     pretty_env_logger::formatted_timed_builder()
//         .format_indent(None)
//         .format(move |f, record| {
//             let ts = f.timestamp();
//
//             let mut level_style = f.style();
//
//             if conf.color {
//                 let color = match record.level() {
//                     Level::Trace => Color::Magenta,
//                     Level::Debug => Color::Blue,
//                     Level::Info => Color::Green,
//                     Level::Warn => Color::Yellow,
//                     Level::Error => Color::Red,
//                 };
//                 level_style.set_color(color).set_bold(true);
//             }
//
//             writeln!(
//                 f,
//                 "[{}][{:>5}][{}] {}",
//                 ts,
//                 level_style.value(record.level()),
//                 record.target(),
//                 record.args()
//             )
//         })
//         .format_timestamp_secs()
//         .filter_level(config.level)
//         .init();
//
//     Ok(())
// }

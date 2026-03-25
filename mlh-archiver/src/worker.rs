use crate::errors;
use crate::file_utils;
use log::{Level, log_enabled};
use nntp::NNTPStream;
use std::{fmt, path::Path, thread::sleep, time::Duration};

pub fn connect_to_nntp(address: String) -> nntp::Result<NNTPStream> {
    let mut nntp_stream = match NNTPStream::connect(address) {
        Ok(stream) => stream,
        Err(e) => {
            return Err(e);
        }
    };

    match nntp_stream.capabilities() {
        Ok(lines) => {
            if log_enabled!(Level::Debug) {
                log::debug!("server capabilities");
                for line in lines.iter() {
                    log::debug!("{}", line);
                }
            }
        }
        Err(e) => log::error!("Failed checking server capabilities: {}", e),
    }
    return Ok(nntp_stream);
}

pub struct Worker {
    id: u8,
    hostname: String,
    nntp_stream: NNTPStream,
    base_output_path: String,
    needs_reconnection: bool,
    receiver: crossbeam_channel::Receiver<String>,
}

impl Worker {
    pub fn new(
        id: u8,
        hostname: String,
        port: u16,
        base_output_path: String,
        receiver: crossbeam_channel::Receiver<String>,
    ) -> Worker {
        let nntp_stream = connect_to_nntp(format!("{}:{}", hostname.clone(), port))
            .expect("Worker should have connected to the server");

        Worker {
            id,
            hostname,
            base_output_path,
            nntp_stream,
            needs_reconnection: false,
            receiver,
        }
    }

    pub fn run(&mut self) -> crate::Result<()> {
        log::info!("W{}: started consumming tasks", self.id);
        loop {
            // check if reconnection is needed before trying to connect
            if self.needs_reconnection {
                log::debug!("W{}: will attempt a reconnection soon", self.id);
                // wait  a minute before trying to reconnect
                std::thread::sleep(Duration::from_secs(60));

                log::info!("W{}: will attempt a reconnection", self.id);
                match self.nntp_stream.re_connect() {
                    Ok(_) => self.needs_reconnection = false,
                    Err(e) => {
                        log::error!(
                            "W{}: attempted reconnection and failed with error {e}",
                            self.id
                        );
                        return Err(errors::Error::NNTP(e));
                    }
                }
            }

            log::info!("W{}: Reading new group from channel", self.id);
            let group_name = match self.receiver.recv() {
                Ok(name) => name,
                Err(crossbeam_channel::RecvError) => {
                    log::warn!("W{}: Channel closed, worker exiting", self.id);
                    return Ok(());
                }
            };
            // let handler_result =
            match self.handle_group(group_name.clone()) {
                Ok(return_status) => {
                    log::info!("W{}: completed a task with: {return_status}", self.id);
                }
                Err(err) => {
                    if nntp::errors::check_network_error(&err) {
                        log::warn!(
                            "W{}: failed with a network error while reading {group_name}. Error {}",
                            self.id,
                            &err
                        );
                        // if connection error was returned, sleep a bit
                        std::thread::sleep(Duration::from_secs(10));
                    } else {
                        log::error!(
                            "W{}: failed while processing {group_name} with error {}",
                            self.id,
                            &err
                        );
                    }

                    // when an error happens, force a reconnection
                    self.needs_reconnection = true;
                    // attempt to close connection
                    match self.nntp_stream.quit() {
                        Ok(_) => {
                            log::debug!("W{}: Connection closed successfully", self.id);
                        }
                        Err(err) => {
                            log::warn!(
                                "W{}: Failed when closing connection with error {err}. Waiting before triggering a reconnection",
                                self.id
                            );
                            std::thread::sleep(Duration::from_secs(5));
                        }
                    }
                }
            };
            // interval between tasks
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    pub fn handle_group(&mut self, group_name: String) -> nntp::Result<WorkerGroupResult> {
        let read_status: ReadStatus = match file_utils::read_yaml::<ReadStatus>(
            format!(
                "{}/{}/__last_article_number",
                self.base_output_path, group_name
            )
            .as_str(),
        ) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("W{}: Error reading status:  {e}", self.id);
                // attempted to read a number from the file, or fallback to 1
                let last_article_number = file_utils::try_read_number(Path::new(
                    format!(
                        "{}/{}/__last_article_number",
                        self.base_output_path, group_name
                    )
                    .as_str(),
                ))
                .unwrap_or(0);
                if last_article_number == 0 {
                    log::info!("W{}: Reading list {group_name} from mail 0", self.id);
                }

                let read_status = ReadStatus {
                    last_email: last_article_number,
                };

                // write ReadStatus
                file_utils::write_yaml(
                    format!(
                        "{}/{}/__last_article_number",
                        self.base_output_path, group_name
                    )
                    .as_str(),
                    &read_status,
                )?;

                read_status
            }
        };

        let last_article_number = read_status.last_email;

        log::info!(
            "W{}: Checking group : {group_name}. Local max ID: {last_article_number}",
            self.id
        );

        match self.nntp_stream.group(&group_name) {
            Ok(group) => {
                log::info!(
                    "W{}: Remote max for {} is {}, local is {}",
                    self.id,
                    group_name,
                    group.high,
                    last_article_number
                );

                if last_article_number < group.high as usize {
                    log::info!("W{}: Reading emails for group : {group_name}.", self.id);
                    // this call may return an IO error,
                    match self.read_new_mails(
                        group_name.clone(),
                        last_article_number.max(group.low as usize),
                        group.high as usize,
                    ) {
                        Ok(num_emails_read) => {
                            return Ok(WorkerGroupResult::Ok(group_name, num_emails_read));
                        }
                        Err(e) => {
                            log::error!("W{}: Failed reading new mails: {e}", self.id);
                            // TODO: return failure instead of error ?
                            return Err(e);
                        }
                    };
                } else {
                    log::info!(
                        "W{}: Checking group : {group_name}. Local max ID: {last_article_number}",
                        self.id
                    );
                    return Ok(WorkerGroupResult::NoNews(group_name));
                }
            }
            Err(e) => {
                log::error!(
                    "W{}: failure connecting to {group_name}, error: {e}",
                    self.id
                );
                return Err(e);
            }
        }
        // Ok(())
    }

    pub fn handle_group_range(
        &mut self,
        group_name: String,
        range: impl Iterator<Item = usize>,
    ) -> nntp::Result<()> {
        log::info!("W{}: Checking group : {group_name}", self.id);

        match self.nntp_stream.group(&group_name) {
            Ok(group) => {
                log::info!(
                    "W{}: Will start collecting mails from range for group {group}",
                    self.id
                );
                for article_number in range {
                    self.read_new_mails(group_name.clone(), article_number, article_number)?;
                }
            }
            Err(e) => {
                log::error!(
                    "W{}: failure connecting to {group_name}, error: {e}",
                    self.id
                );
                return Err(e);
            }
        }
        Ok(())
    }

    // read_new_mails checks for mails in an inclusive range between low and high
    fn read_new_mails(
        &mut self,
        group_name: String,
        low: usize,
        high: usize,
    ) -> nntp::Result<usize> {
        // take the last_article_number or the "low"" result for the group
        let mut num_emails_read: usize = 0;
        for current_mail in low..=high {
            match self.get_raw_article_by_number_retryable(current_mail as isize, 3) {
                Ok(raw_article) => {
                    file_utils::write_lines_file(
                        Path::new(
                            format!(
                                "{}/{}/{}.eml",
                                self.base_output_path, group_name, current_mail
                            )
                            .as_str(),
                        ),
                        raw_article,
                    )
                    .unwrap();
                    num_emails_read += 1;

                    // write ReadStatus
                    file_utils::write_yaml(
                        format!(
                            "{}/{}/__last_article_number",
                            self.base_output_path, group_name
                        )
                        .as_str(),
                        &ReadStatus {
                            last_email: current_mail,
                        },
                    )?;
                }
                Err(e) => {
                    match e {
                        nntp::NNTPError::ArticleUnavailable => {
                            file_utils::append_line_to_file(
                                Path::new(
                                    format!("{}/{}/__errors", self.base_output_path, group_name)
                                        .as_str(),
                                ),
                                format!("{current_mail},{e}").as_str(),
                            )
                            .unwrap();
                            log::warn!(
                                "W{}: Email with number {current_mail} unavailable",
                                self.id
                            );
                        }
                        _ => return Err(e),
                    }
                    // // TODO: should the program signal a need to reconnect here or upstream ?
                    // return Err(e);
                }
            }

            log::info!(
                "W{}: {group_name} {}/{} ({:.2}%)",
                self.id,
                current_mail,
                high,
                (current_mail as f64 / high as f64 * 100.0)
            );
        }
        return Ok(num_emails_read);
    }

    fn get_raw_article_by_number_retryable(
        &mut self,
        mail_num: isize,
        max_retries: usize,
    ) -> nntp::Result<Vec<String>> {
        let mut attempts = 0;
        let retry_delay_ms = 600;
        loop {
            match self.nntp_stream.raw_article_by_number(mail_num) {
                Ok(raw_article) => {
                    return Ok(raw_article);
                }
                Err(e) => {
                    log::warn!(
                        "W{}: Failed reading article '{}' from '{}' : {}",
                        self.id,
                        mail_num,
                        self.hostname,
                        e
                    );
                    attempts += 1;
                    if attempts > max_retries {
                        // Return the last error after max retries
                        return Err(e);
                    }
                    log::warn!(
                        "W{}: Retrying in {}ms...",
                        self.id,
                        (retry_delay_ms * (attempts + 1))
                    );
                    sleep(Duration::from_millis(
                        (retry_delay_ms * (attempts + 1)) as u64,
                    ));
                }
            }
        }
    }
}

pub enum WorkerGroupResult {
    Ok(String, usize),
    NoNews(String),
    // Failed(String),
}

impl fmt::Display for WorkerGroupResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            WorkerGroupResult::Ok(group_name, num_emails) => {
                write!(
                    f,
                    "Collected {num_emails} new e-mails from {:?}",
                    group_name
                )
            }
            WorkerGroupResult::NoNews(group_name) => {
                write!(f, "No New e-mails from {:?}", group_name)
            }
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct ReadStatus {
    pub last_email: usize,
}

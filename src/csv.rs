use crate::pull_request::{self, EventDetail};

struct RecordRow {
    timestamp: String,
    actor: Option<String>,
    event_type: String,
    delay: f64,
    pr_number: i64,
    pr_size: i64,
    from_teams: String,
    to_teams: String,
    review_state: Option<String>,
    review_comments: Option<i64>,
}

impl std::fmt::Display for RecordRow {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let blank = String::from("");
        write!(
            f,
            "{}\t{}\t{}\t{:.3}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.timestamp,
            self.actor.as_ref().unwrap_or(&blank),
            self.event_type,
            self.delay,
            self.pr_number,
            self.pr_size,
            self.from_teams,
            self.to_teams,
            self.review_state.as_ref().unwrap_or(&blank),
            self.review_comments.unwrap_or(0)
        )
    }
}

impl std::fmt::Display for pull_request::ReviewStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                pull_request::ReviewStatus::Pending => "Pending",
                pull_request::ReviewStatus::Commented => "Commented",
                pull_request::ReviewStatus::Approved => "Approved",
                pull_request::ReviewStatus::ChangesRequested => "Changes requested",
                pull_request::ReviewStatus::Dismissed => "Dismissed",
            }
        )
    }
}

pub fn print_header() {
    println!("timestamp\tactor\tevent_type\tdelay\tpr_number\tpr_size\tfrom_teams\tto_teams\treview_state\treview_comments");
}

pub fn print_pr(pr: pull_request::PullRequest) {
    for event in &pr.events {
        let from_teams = pr.authoring_teams.join(",");
        let to_teams = pr.reviewing_teams.join(",");

        let row = match &event.details {
            EventDetail::Open => RecordRow {
                actor: event.actor.map(String::from),
                timestamp: String::from(event.timestamp),
                delay: event.delay,
                event_type: String::from("OPEN"),
                pr_number: pr.number,
                pr_size: pr.diff_size,
                from_teams: from_teams,
                to_teams: to_teams,
                review_state: None,
                review_comments: None,
            },
            EventDetail::Commit(_) => RecordRow {
                actor: event.actor.map(String::from),
                timestamp: String::from(event.timestamp),
                delay: event.delay,
                event_type: String::from("COMMIT"),
                pr_number: pr.number,
                pr_size: pr.diff_size,
                from_teams: from_teams,
                to_teams: to_teams,
                review_state: None,
                review_comments: None,
            },
            EventDetail::Review(review) => {
                let state = format!("{}", review.state);
                let state_str = state.as_str();

                RecordRow {
                    actor: event.actor.map(String::from),
                    timestamp: String::from(event.timestamp),
                    delay: event.delay,
                    event_type: String::from("REVIEW"),
                    pr_number: pr.number,
                    pr_size: pr.diff_size,
                    from_teams: from_teams,
                    to_teams: to_teams,
                    review_state: Some(String::from(state_str)),
                    review_comments: Some(review.comment_count),
                }
            }
            EventDetail::Merged => RecordRow {
                actor: event.actor.map(String::from),
                timestamp: String::from(event.timestamp),
                delay: event.delay,
                event_type: String::from("MERGED"),
                pr_number: pr.number,
                pr_size: pr.diff_size,
                from_teams: from_teams,
                to_teams: to_teams,
                review_state: None,
                review_comments: None,
            },
            EventDetail::Closed => RecordRow {
                actor: event.actor.map(String::from),
                timestamp: String::from(event.timestamp),
                delay: event.delay,
                event_type: String::from("CLOSED"),
                pr_number: pr.number,
                pr_size: pr.diff_size,
                from_teams: from_teams,
                to_teams: to_teams,
                review_state: None,
                review_comments: None,
            },
        };

        println!("{}", row);
    }
}

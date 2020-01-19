use chrono::DateTime;
use serde_json::Value;
use std::collections::HashMap;

type Hours = f64;
type BusinessHours = u32;

#[derive(Debug)]
pub struct PullRequest<'a> {
    pub number: i64,
    pub title: &'a str,
    pub diff_size: i64,
    pub author: &'a str,
    pub reviewers: Vec<&'a str>,
    pub authoring_teams: Vec<&'a str>,
    pub reviewing_teams: Vec<&'a str>,
    pub events: Vec<PullRequestEvent<'a>>,
}

#[derive(Debug)]
pub struct PullRequestEvent<'a> {
    pub actor: Option<&'a str>,
    pub teams: Vec<&'a str>,
    pub timestamp: &'a str,
    pub delay: Hours,
    pub delay_workhours: BusinessHours,
    pub details: EventDetail<'a>,
}

#[derive(Debug)]
pub enum EventDetail<'a> {
    Open,
    Commit(PullRequestCommit<'a>),
    Review(PullRequestReview),
    Merged,
    Closed,
}

#[derive(Debug)]
pub struct PullRequestCommit<'a> {
    sha: &'a str,
}

#[derive(Debug)]
pub struct PullRequestReview {
    pub state: ReviewStatus,
    pub comment_count: i64,
}

#[derive(Debug)]
pub enum ReviewStatus {
    Pending,
    Commented,
    Approved,
    ChangesRequested,
    Dismissed,
}

impl ReviewStatus {
    fn from_str(state: &str) -> ReviewStatus {
        match state {
            "PENDING" => ReviewStatus::Pending,
            "COMMENTED" => ReviewStatus::Commented,
            "APPROVED" => ReviewStatus::Approved,
            "CHANGES_REQUESTED" => ReviewStatus::ChangesRequested,
            "DISMISSED" => ReviewStatus::Dismissed,
            _ => panic!("Unrecognised Pull Request review state {}", state),
        }
    }
}

fn build_event<'a>(
    item: &'a Value,
    teams_by_user: &HashMap<&str, Vec<&'a str>>,
) -> PullRequestEvent<'a> {
    let typename = item["__typename"].as_str().unwrap();

    let (timestamp, actor, details) = match typename {
        "PullRequestCommit" => (
            item["commit"]["committedDate"].as_str().unwrap(),
            item["commit"]["author"]["user"]["login"].as_str(),
            EventDetail::Commit(PullRequestCommit {
                sha: item["commit"]["oid"].as_str().unwrap(),
            }),
        ),
        "PullRequestReview" => (
            item["publishedAt"].as_str().unwrap(),
            item["author"]["login"].as_str(),
            EventDetail::Review(PullRequestReview {
                comment_count: item["comments"]["totalCount"].as_i64().unwrap(),
                state: ReviewStatus::from_str(item["state"].as_str().unwrap()),
            }),
        ),
        "MergedEvent" => (
            item["createdAt"].as_str().unwrap(),
            item["actor"]["login"].as_str(),
            EventDetail::Merged,
        ),
        "ClosedEvent" => (
            item["createdAt"].as_str().unwrap(),
            item["actor"]["login"].as_str(),
            EventDetail::Closed,
        ),
        _ => panic!("Unrecognised Pull Request event type {}", typename),
    };
    PullRequestEvent {
        timestamp: timestamp,
        actor: actor,
        teams: users_teams(&[actor.unwrap_or("")], teams_by_user),
        delay: 0.0,
        delay_workhours: 0, // fix
        details: details,
    }
}

fn users_teams<'a>(users: &[&str], teams_by_user: &HashMap<&str, Vec<&'a str>>) -> Vec<&'a str> {
    let empty = Vec::new();
    let mut teams = users
        .iter()
        .flat_map(|u| teams_by_user.get(u).unwrap_or(&empty))
        .map(|it| *it)
        .collect::<Vec<&str>>();

    teams.sort();
    teams.dedup();

    teams
}

pub fn build<'a>(
    json: &'a Value,
    teams_by_user: &'a HashMap<&'a str, Vec<&'a str>>,
) -> PullRequest<'a> {
    let author = json["author"]["login"].as_str().unwrap();

    // start event list

    let timeline_items = json["timelineItems"]["nodes"].as_array().unwrap();
    let mut events: Vec<PullRequestEvent<'a>> = Vec::with_capacity(timeline_items.len());
    let mut reviewers: Vec<&str> = Vec::with_capacity(10);
    // seed with first event

    let open_event = PullRequestEvent {
        timestamp: json["createdAt"].as_str().unwrap(),
        actor: Some(author),
        teams: users_teams(&[author], teams_by_user),
        delay: 0.0,
        delay_workhours: 0,
        details: EventDetail::Open,
    };
    events.push(open_event);

    for item in timeline_items {
        let event = build_event(item, teams_by_user);

        if let EventDetail::Review(_) = event.details {
            if let Some(actor) = event.actor {
                reviewers.push(actor);
            }
        }

        events.push(event);
    }
    reviewers.sort();
    reviewers.dedup();

    let reviewing_teams = users_teams(&reviewers[..], teams_by_user);

    events.sort_by_key(|e| DateTime::parse_from_rfc3339(e.timestamp).unwrap());

    for i in 1..events.len() {
        let prev_time = DateTime::parse_from_rfc3339(events.get(i - 1).unwrap().timestamp).unwrap();
        let cur_time = DateTime::parse_from_rfc3339(events.get(i).unwrap().timestamp).unwrap();

        let duration = (cur_time - prev_time).num_minutes() as f64 / 60.0;
        events.get_mut(i).unwrap().delay = duration;
    }

    let pr = PullRequest {
        number: json["number"].as_i64().unwrap(),
        title: json["title"].as_str().unwrap(),
        author: author,
        authoring_teams: users_teams(&[author], teams_by_user),
        reviewers: reviewers,
        reviewing_teams: reviewing_teams,
        diff_size: json["additions"].as_i64().unwrap() + json["deletions"].as_i64().unwrap(),
        events: events,
    };

    return pr;
}

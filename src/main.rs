extern crate chrono;

use chrono::DateTime;
use gh::client::Github;
use gh::query::Query;
use github_gql as gh;
use serde_json::Value;
use std::collections::HashMap;

// Data model

type Hours = f64;
type BusinessHours = u32;

#[derive(Debug)]
struct PullRequestCommit<'a> {
    sha: &'a str,
}

#[derive(Debug)]
enum ReviewStatus {
    Pending,
    Commented,
    Approved,
    ChangesRequested,
    Dismissed,
}

impl std::fmt::Display for ReviewStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ReviewStatus::Pending => "Pending",
                ReviewStatus::Commented => "Commented",
                ReviewStatus::Approved => "Approved",
                ReviewStatus::ChangesRequested => "Changes requested",
                ReviewStatus::Dismissed => "Dismissed",
            }
        )
    }
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

#[derive(Debug)]
struct PullRequestReview {
    state: ReviewStatus,
    comment_count: i64,
}

#[derive(Debug)]
enum EventDetail<'a> {
    Open,
    Commit(PullRequestCommit<'a>),
    Review(PullRequestReview),
    Merged,
    Closed,
}

#[derive(Debug)]
struct PullRequestEvent<'a> {
    pub actor: Option<&'a str>,
    pub teams: Vec<&'a str>,
    pub timestamp: &'a str,
    pub delay: Hours,
    pub delay_workhours: BusinessHours,
    pub details: EventDetail<'a>,
}

#[derive(Debug)]
struct PullRequest<'a> {
    number: i64,
    title: &'a str,
    diff_size: i64,
    author: &'a str,
    reviewers: Vec<&'a str>,
    authoring_teams: Vec<&'a str>,
    reviewing_teams: Vec<&'a str>,
    events: Vec<PullRequestEvent<'a>>,
}

// GraphQL fetching

fn fetch_teams(github: &mut Github) -> Vec<Value> {
    let query = Query::new_raw(
        "{
            organization(login:\"redbadger\") {
                teams(first: 20, query:\"cdk-wave-\") {
                nodes {
                    name
                    members {
                        nodes {
                            login
                        }
                    }
                }
                }
            }
        }",
    );

    let (_, _, json) = github.query::<Value>(&query).unwrap();
    let data = json.unwrap();

    return data["data"]["organization"]["teams"]["nodes"]
        .as_array()
        .unwrap()
        .clone();
}

fn fetch_pull_request_query(first: usize, after: &Option<String>) -> Query {
    let after = match after {
        Some(cursor) => format!("\"{}\"", cursor),
        None => String::from("null"),
    };

    Query::new_raw(format!(
        r#"
            {{
                repository(owner:"redbadger", name: "pagofx") {{
                    pullRequests(orderBy: {{field: CREATED_AT,direction: DESC}}, first: {}, after: {}) {{
                    pageInfo {{
                        endCursor
                        hasNextPage
                    }}
                    nodes {{
                        number
                        title
                        additions
                        deletions
                        createdAt
                        author {{ 
                            login 
                        }}
                        timelineItems(itemTypes: [PULL_REQUEST_COMMIT, PULL_REQUEST_REVIEW, MERGED_EVENT, CLOSED_EVENT], first: 200) {{
                        nodes {{
                            __typename
                            ... on PullRequestCommit {{
                                commit {{
                                    oid
                                    committedDate
                                    author {{
                                        user {{
                                            login
                                        }}
                                    }}
                                }}
                            }}
                            ...on PullRequestReview {{
                                publishedAt
                                state
                                author {{
                                    login
                                }}
                                comments {{
                                    totalCount
                                }}
                            }}
                            ...on MergedEvent {{
                                createdAt
                                actor {{
                                    login
                                }}
                            }}
                            ...on ClosedEvent {{
                                createdAt
                                actor {{
                                    login
                                }}
                            }}
                        }}
                        }}
                    }}
                    }}
                }}
            }}"#,
        first, after
    ))
}

fn fetch_pull_requests(github: &mut Github, total: usize) -> Vec<Value> {
    let mut limit = if total > 100 { 100 } else { total };
    let mut after = None;

    let mut pull_requests = Vec::<Value>::with_capacity(limit);

    for _ in 0..=(total / 100 + 1) {
        let query = fetch_pull_request_query(limit, &after);

        println!(
            "Fetching Pull Requests... {}/{}",
            pull_requests.len() + limit,
            total
        );
        let (_, _, json) = github.query::<Value>(&query).unwrap();

        let mut data = json.unwrap();
        let prs = data["data"]["repository"]["pullRequests"]
            .as_object_mut()
            .unwrap();

        let items = prs["nodes"].as_array_mut().unwrap();
        pull_requests.append(items);

        let remaining = total - pull_requests.len();
        limit = if (remaining) > 100 { 100 } else { remaining };

        if limit < 1 || prs["pageInfo"]["hasNextPage"].as_bool().unwrap() == false {
            break;
        }

        after = Some(String::from(prs["pageInfo"]["endCursor"].as_str().unwrap()));
    }
    return pull_requests;
}

// Processing JSON into data

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

fn index_teams_by_users(teams: &Vec<Value>) -> HashMap<&str, Vec<&str>> {
    let mut index = HashMap::new();
    for team in teams {
        let team_name = team["name"].as_str().unwrap();
        let members: Vec<_> = team["members"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|member| member["login"].as_str().unwrap())
            .collect();

        for member in members {
            let members_teams = index.entry(member).or_insert(Vec::new());
            members_teams.push(team_name);
        }
    }

    return index;
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

fn build_pull_request<'a>(
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

// Writing tabular output

fn main() {
    let mut github = Github::new("92e86a66b4f38662fbb67d6560a419808d891b62").unwrap();

    let teams = fetch_teams(&mut github);
    let team_from_user = index_teams_by_users(&teams);

    let pull_requests_json = fetch_pull_requests(&mut github, 400);
    let pull_requests: Vec<_> = pull_requests_json
        .iter()
        .map(|pr| build_pull_request(pr, &team_from_user))
        .collect();

    for pr in &pull_requests {
        println!(
            "PR #{} ({} lines). By {} ({}), reviewed by {} ({}).",
            pr.number,
            pr.diff_size,
            pr.author,
            pr.authoring_teams.join(", "),
            pr.reviewers.join(", "),
            pr.reviewing_teams.join(", ")
        );
        for event in &pr.events {
            match &event.details {
                EventDetail::Open => println!(
                    "- Opened by {} on {} (after {:.2} hours)",
                    event.actor.unwrap_or(""),
                    event.timestamp,
                    event.delay
                ),
                EventDetail::Commit(_) => println!(
                    "- Committed to by {} on {} (after {:.2} hours)",
                    event.actor.unwrap_or(""),
                    event.timestamp,
                    event.delay
                ),
                EventDetail::Review(review) => println!(
                    "- Reviewed by {} ({}) on {} (after {:.2} hours), {} comments",
                    event.actor.unwrap_or(""),
                    review.state,
                    event.timestamp,
                    event.delay,
                    review.comment_count
                ),
                EventDetail::Merged => println!(
                    "- Merged by {} on {} (after {:.2} hours)",
                    event.actor.unwrap_or(""),
                    event.timestamp,
                    event.delay
                ),
                EventDetail::Closed => println!(
                    "- Closed by {} on {} (after {:.2} hours)",
                    event.actor.unwrap_or(""),
                    event.timestamp,
                    event.delay
                ),
            }
        }
        println!("");
    }
}

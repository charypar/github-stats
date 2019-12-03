use gh::client::Github;
use gh::query::Query;
use github_gql as gh;
use serde_json::Value;

// Data model

type ISODate = String;
type BusinessHours = u32;

struct PullRequestCommit {
    sha: String,
}

enum ReviewStatus {
    Pending,
    Commented,
    Approved,
    ChangesRequested,
    Dismissed,
}

struct PullRequestReview {
    state: ReviewStatus,
}

enum EventDetail {
    Open,
    Commit(PullRequestCommit),
    Review(PullRequestReview),
    Merged,
    Closed,
}

struct PullRequestEvent {
    pub actor: String,
    pub team: String,
    pub timestamp: ISODate,
    pub duration: BusinessHours,
    pub details: EventDetail,
}

struct PullRequest {
    number: usize,
    title: String,
    additions: u32,
    deletions: u32,
    suggested_reviewers: Vec<String>,
    events: Vec<PullRequestEvent>,
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
                        createdAt
                        author {{ 
                            login 
                        }}
                        suggestedReviewers {{
                            reviewer {{
                                login
                            }}
                        }}
                        timelineItems(itemTypes: [PULL_REQUEST_COMMIT, PULL_REQUEST_REVIEW, MERGED_EVENT, CLOSED_EVENT], first: 200) {{
                        nodes {{
                            __typename
                            ... on PullRequestCommit {{
                                commit {{
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

// Writing tabular output

fn main() {
    let mut github = Github::new("92e86a66b4f38662fbb67d6560a419808d891b62").unwrap();

    let teams = fetch_teams(&mut github);
    let pull_requests = fetch_pull_requests(&mut github, 30);

    for team in teams {
        let members: Vec<_> = team["members"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["login"].as_str().unwrap())
            .collect();

        println!("Team {}: {:?}", team["name"], members);
    }

    for pr in &pull_requests {
        println!(
            "PR {}: {} ({} events)",
            pr["number"],
            pr["title"],
            pr["timelineItems"]["nodes"].as_array().unwrap().len()
        );
    }
    println!("Total PRs: {}", pull_requests.len())
}

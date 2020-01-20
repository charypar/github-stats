use gh::client::Github;
use gh::query::Query;
use github_gql as gh;

use serde_json::Value;

pub fn fetch_teams(github: &mut Github, org: &str, team_query: &str) -> Vec<Value> {
    let query = Query::new_raw(format!(
        r#"{{
            organization(login: "{}") {{
                teams(first: 20, query: "{}") {{
                    nodes {{
                        name
                        members {{
                            nodes {{
                                login
                            }}
                        }}
                    }}
                }}
            }}
        }}
        "#,
        org, team_query
    ));

    let (_, _, json) = github.query::<Value>(&query).unwrap();
    let data = json.unwrap();

    return data["data"]["organization"]["teams"]["nodes"]
        .as_array()
        .unwrap()
        .clone();
}

pub struct PullRequestsIter<'a> {
    github: &'a mut Github,
    org: &'a str,
    repo: &'a str,
    total: usize,
    remaining: isize,
    page_info: PageInfo,
}

struct PageInfo {
    has_next_page: bool,
    end_cursor: Option<String>,
}

impl std::iter::Iterator for PullRequestsIter<'_> {
    type Item = Vec<Value>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.page_info.has_next_page || self.remaining < 1 {
            return None;
        }

        eprintln!(
            "Fetching Pull Requests... {} of {} remaining",
            self.remaining, self.total
        );

        let limit = std::cmp::min(std::cmp::max(self.remaining, 0), 100) as usize;
        let after = self.page_info.end_cursor.as_ref();

        let (prs, page_info) = fetch_pull_requests(self.github, self.org, self.repo, limit, after);

        self.remaining -= 100;
        self.page_info = page_info;

        Some(prs)
    }
}

pub fn pull_requests<'a>(
    github: &'a mut Github,
    org: &'a str,
    repo: &'a str,
    total: usize,
) -> PullRequestsIter<'a> {
    PullRequestsIter {
        github,
        org,
        repo,
        total,
        remaining: total as isize,
        page_info: PageInfo {
            has_next_page: true,
            end_cursor: None,
        },
    }
}

fn fetch_pull_requests(
    github: &mut Github,
    org: &str,
    repo: &str,
    limit: usize,
    after: Option<&String>,
) -> (Vec<Value>, PageInfo) {
    let query = fetch_pull_request_query(org, repo, limit, after);

    let (_, _, json) = github.query::<Value>(&query).unwrap();

    let data = json.unwrap();
    let pr_node = data["data"]["repository"]["pullRequests"]
        .as_object()
        .unwrap(); // FIXME Occasionally panics here

    let pull_requests = pr_node["nodes"].as_array().unwrap().clone();

    let page_info = PageInfo {
        has_next_page: pr_node["pageInfo"]["hasNextPage"].as_bool().unwrap(),
        end_cursor: pr_node["pageInfo"]["endCursor"]
            .as_str()
            .map(|s| s.to_owned()),
    };

    return (pull_requests, page_info);
}

fn fetch_pull_request_query(org: &str, repo: &str, first: usize, after: Option<&String>) -> Query {
    let after = match after {
        Some(cursor) => format!("\"{}\"", cursor),
        None => String::from("null"),
    };

    Query::new_raw(format!(
        r#"
          {{
              repository(owner:"{}", name: "{}") {{
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
        org, repo, first, after
    ))
}

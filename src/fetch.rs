use gh::client::Github;
use gh::query::Query;
use github_gql as gh;

use serde_json::Value;

pub fn fetch_teams(github: &mut Github) -> Vec<Value> {
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

pub fn fetch_pull_requests(github: &mut Github, total: usize) -> Vec<Value> {
    let mut limit = if total > 100 { 100 } else { total };
    let mut after = None;

    let mut pull_requests = Vec::<Value>::with_capacity(limit);

    for _ in 0..=(total / 100 + 1) {
        let query = fetch_pull_request_query(limit, &after);

        eprintln!(
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

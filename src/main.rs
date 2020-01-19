extern crate chrono;

mod csv;
mod fetch;
mod pull_request;

use github_gql::client::Github;
use serde_json::Value;
use std::collections::HashMap;

use fetch::{fetch_pull_requests, fetch_teams};

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

fn main() {
    let mut github = Github::new("92e86a66b4f38662fbb67d6560a419808d891b62").unwrap();

    let teams = fetch_teams(&mut github);
    let team_from_user = index_teams_by_users(&teams);

    let pull_requests_json = fetch_pull_requests(&mut github, 600);

    csv::print_header();
    for json in pull_requests_json {
        csv::print_pr(pull_request::build(&json, &team_from_user))
    }
}

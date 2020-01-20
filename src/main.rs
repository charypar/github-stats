extern crate chrono;

mod csv;
mod fetch;
mod pull_request;

use github_gql::client::Github;
use serde_json::Value;
use std::collections::HashMap;

use fetch::{fetch_teams, pull_requests};

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
    let org = "redbadger";
    let repo = "pagofx";
    let teams = "cdk-wave-";
    let limit = 600;

    let mut github = Github::new("92e86a66b4f38662fbb67d6560a419808d891b62").unwrap();

    let teams = fetch_teams(&mut github, org, teams);
    let team_from_user = index_teams_by_users(&teams);

    csv::print_header();
    for batch in pull_requests(&mut github, org, repo, limit) {
        for json in batch {
            csv::print_pr(pull_request::build(&json, &team_from_user))
        }
    }
}

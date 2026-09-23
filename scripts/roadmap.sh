#!/usr/bin/env bash
#
# Manage the smartgrep roadmap board (GitHub Project "smartgrep roadmap").
# Every board item is a real issue in rohitkg98/smartgrep; board order = priority.
#
# Usage:
#   scripts/roadmap.sh list                               # items in priority order
#   scripts/roadmap.sh add "<title>" <body-file> [--after <N>|--top]
#   scripts/roadmap.sh status <N> "Todo"|"In Progress"|"Done"
#   scripts/roadmap.sh move <N> --after <M>|--top
#
# <N>/<M> are issue numbers. Requires `gh` authenticated with the `project` scope
# (`gh auth refresh -s project`).
#
set -euo pipefail

OWNER="rohitkg98"
REPO="rohitkg98/smartgrep"
PROJECT_NUMBER=2

die() { echo "error: $*" >&2; exit 1; }

project_id() {
    gh project view "$PROJECT_NUMBER" --owner "$OWNER" --format json --jq .id
}

status_field_json() {
    gh project field-list "$PROJECT_NUMBER" --owner "$OWNER" --format json \
        --jq '.fields[] | select(.name == "Status")'
}

# All items as TSV: item_id, issue_number, status, title (in board order).
items_tsv() {
    gh api graphql -F owner="$OWNER" -F number="$PROJECT_NUMBER" -f query='
      query($owner: String!, $number: Int!) {
        user(login: $owner) { projectV2(number: $number) { items(first: 100) { nodes {
          id
          content { ... on Issue { number title } ... on DraftIssue { title } }
          fieldValueByName(name: "Status") { ... on ProjectV2ItemFieldSingleSelectValue { name } }
        } } } }
      }' --jq '.data.user.projectV2.items.nodes[]
        | [.id, (.content.number // "draft"), (.fieldValueByName.name // "-"), .content.title] | @tsv'
}

item_id_for_issue() {
    local id
    id=$(items_tsv | awk -F'\t' -v n="$1" '$2 == n { print $1 }')
    [[ -n "$id" ]] || die "issue #$1 is not on the board"
    echo "$id"
}

set_position() { # item_id, after_item_id ("" = top)
    local pid; pid=$(project_id)
    if [[ -z "$2" ]]; then
        gh api graphql -f p="$pid" -f i="$1" -f query='
          mutation($p: ID!, $i: ID!) { updateProjectV2ItemPosition(input: {projectId: $p, itemId: $i}) { clientMutationId } }' >/dev/null
    else
        gh api graphql -f p="$pid" -f i="$1" -f a="$2" -f query='
          mutation($p: ID!, $i: ID!, $a: ID!) { updateProjectV2ItemPosition(input: {projectId: $p, itemId: $i, afterId: $a}) { clientMutationId } }' >/dev/null
    fi
}

set_status() { # item_id, status name
    local field option
    field=$(status_field_json)
    option=$(jq -r --arg s "$2" '.options[] | select(.name == $s) | .id' <<<"$field")
    [[ -n "$option" ]] || die "unknown status '$2' (valid: $(jq -r '[.options[].name] | join(", ")' <<<"$field"))"
    gh project item-edit --id "$1" --project-id "$(project_id)" \
        --field-id "$(jq -r .id <<<"$field")" --single-select-option-id "$option" >/dev/null
}

apply_position_flag() { # item_id, flag, [issue]
    case "${2:-}" in
        --top)   set_position "$1" "" ;;
        --after) [[ -n "${3:-}" ]] || die "--after needs an issue number"
                 set_position "$1" "$(item_id_for_issue "$3")" ;;
        "")      ;;
        *)       die "unknown flag '$2'" ;;
    esac
}

cmd="${1:-}"; shift || true
case "$cmd" in
    list)
        items_tsv | awk -F'\t' '{ printf "%2d. #%-6s %-12s %s\n", NR, $2, $3, $4 }'
        ;;
    add)
        [[ $# -ge 2 ]] || die "usage: add \"<title>\" <body-file> [--after <N>|--top]"
        title="$1"; body_file="$2"; shift 2
        [[ -f "$body_file" ]] || die "body file not found: $body_file"
        url=$(gh issue create --repo "$REPO" --title "$title" --body-file "$body_file")
        item=$(gh project item-add "$PROJECT_NUMBER" --owner "$OWNER" --url "$url" --format json --jq .id)
        set_status "$item" "Todo"
        apply_position_flag "$item" "${1:-}" "${2:-}"
        echo "$url"
        ;;
    status)
        [[ $# -eq 2 ]] || die "usage: status <N> \"Todo\"|\"In Progress\"|\"Done\""
        set_status "$(item_id_for_issue "$1")" "$2"
        ;;
    move)
        [[ $# -ge 2 ]] || die "usage: move <N> --after <M>|--top"
        apply_position_flag "$(item_id_for_issue "$1")" "$2" "${3:-}"
        ;;
    *)
        sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'
        [[ -z "$cmd" ]] || exit 1
        ;;
esac

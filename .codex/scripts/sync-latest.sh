#!/bin/sh

set -eu

resolve_detached_upstream() {
    candidates="$(
        git for-each-ref \
            --format='%(refname:short) %(symref)' \
            --points-at=HEAD \
            refs/remotes/origin \
            | awk '$2 == "" { print $1 }'
    )"
    default_upstream="$(
        git symbolic-ref --quiet --short refs/remotes/origin/HEAD || true
    )"

    if [ -n "$default_upstream" ] \
        && printf '%s\n' "$candidates" | grep -Fqx "$default_upstream"
    then
        printf '%s\n' "$default_upstream"
        return
    fi

    candidate_count="$(printf '%s\n' "$candidates" | awk 'NF { count++ } END { print count + 0 }')"

    if [ "$candidate_count" -eq 1 ]; then
        printf '%s\n' "$candidates"
        return
    fi

    if [ "$candidate_count" -eq 0 ]; then
        echo 'Could not infer the source branch for this detached worktree.' >&2
    else
        echo 'Several remote branches point at this detached worktree; the source branch is ambiguous.' >&2
    fi

    exit 1
}

branch="$(git symbolic-ref --quiet --short HEAD || true)"

if [ -n "$branch" ]; then
    upstream="$(git for-each-ref --format='%(upstream:short)' "refs/heads/$branch")"

    if [ -z "$upstream" ] && git show-ref --verify --quiet "refs/remotes/origin/$branch"; then
        upstream="origin/$branch"
    fi

    if [ -z "$upstream" ]; then
        echo "No upstream found for $branch." >&2
        exit 1
    fi
else
    # Codex worktrees use a detached HEAD. Resolve the source while the
    # pre-fetch remote-tracking ref still points at the starting commit.
    upstream="$(resolve_detached_upstream)"
fi

git fetch --prune origin
git merge --ff-only "$upstream"

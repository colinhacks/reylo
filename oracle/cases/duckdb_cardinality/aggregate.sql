select count(*) as n, max(seen_at) as latest from events where account_id = :id

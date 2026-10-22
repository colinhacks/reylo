with a as (select p.published_at as at from posts p union all select null::timestamptz) select at from a

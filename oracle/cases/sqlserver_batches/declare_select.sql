declare @since datetime2(3) = dateadd(day, -@days, cast('2026-07-01' as datetime2(3)))
select id, total, upper(note) as shout from orders where placed_at > @since

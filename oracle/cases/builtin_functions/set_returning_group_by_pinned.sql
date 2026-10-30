-- A set-returning call in GROUP BY splits the one group the WHERE pins.
select kind, count(*) as n from events where kind = 'a' group by kind, unnest('{1,1,3}'::int[])

select date_trunc('month', published_at) as month, sum(count(*)) over (order by date_trunc('month', published_at)) as running from posts group by date_trunc('month', published_at)

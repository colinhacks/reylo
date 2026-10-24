select date_format(published_at, '%Y-%m') as month, sum(count(*)) over (order by date_format(published_at, '%Y-%m')) as running from posts group by date_format(published_at, '%Y-%m')

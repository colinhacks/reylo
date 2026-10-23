with paid_orders as (
  select
    o.id,
    o.user_id,
    o.placed_at,
    sum(oi.quantity * oi.unit_price_cents)::int as total_cents,
    count(*)::int as line_count
  from orders o
  join order_items oi on oi.order_id = o.id
  where o.org_id = :org_id
    and o.status in ('paid', 'shipped')
    and o.placed_at >= now() - make_interval(days => :days)
  group by o.id
),
customer_totals as (
  select
    user_id,
    count(*)::int as order_count,
    sum(total_cents)::int as lifetime_cents,
    round(avg(total_cents))::int as avg_order_cents,
    percentile_cont(0.5) within group (order by total_cents) as median_order_cents,
    sum(line_count)::int as items_bought,
    max(placed_at) as last_order_at
  from paid_orders
  group by user_id
),
ranked as (
  select
    ct.*,
    rank() over (order by lifetime_cents desc)::int as spend_rank,
    ntile(4) over (order by lifetime_cents desc) as quartile
  from customer_totals ct
)
select
  u.id,
  u.name,
  u.email,
  u.role,
  org.name as org,
  org.plan,
  r.order_count,
  r.items_bought,
  r.lifetime_cents,
  r.avg_order_cents,
  r.median_order_cents,
  r.spend_rank,
  r.lifetime_cents - lag(r.lifetime_cents) over (order by r.spend_rank) as gap_to_next_cents,
  case r.quartile
    when 1 then 'whale'
    when 2 then 'regular'
    else 'casual'
  end as segment,
  r.last_order_at,
  (
    select p.name
    from order_items oi
    join orders ord on ord.id = oi.order_id
    join products p on p.id = oi.product_id
    where ord.user_id = u.id
    group by p.id, p.name
    order by sum(oi.quantity) desc, p.name
    limit 1
  ) as favorite_product,
  (
    select json_agg(json_build_object(
      'sku', p.sku,
      'name', p.name,
      'quantity', oi.quantity,
      'subtotal_cents', oi.quantity * oi.unit_price_cents,
      'tags', p.tags
    ) order by p.name)
    from order_items oi
    join products p on p.id = oi.product_id
    where oi.order_id = (
      select po.id
      from paid_orders po
      where po.user_id = u.id
      order by po.placed_at desc
      limit 1
    )
  ) as last_order_items,
  (
    select count(*)::int
    from reviews rv
    where rv.user_id = u.id
  ) as review_count,
  (
    select round(avg(rv.rating), 2)::float8
    from reviews rv
    where rv.user_id = u.id
  ) as avg_rating,
  exists (
    select 1
    from orders refunded
    where refunded.user_id = u.id
      and refunded.status = 'refunded'
  ) as has_refund
from ranked r
join users u on u.id = r.user_id
join orgs org on org.id = u.org_id
where r.spend_rank <= :top_n
order by r.spend_rank, u.name

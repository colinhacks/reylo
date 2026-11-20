select total * 2 as doubled, total / 3 as third, -total as negated, total + discount as odd from orders o join lines l on l.order_id = o.id

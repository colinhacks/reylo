if @flag = 1
  select id, total from orders
else
  select id, total from orders where total > 0

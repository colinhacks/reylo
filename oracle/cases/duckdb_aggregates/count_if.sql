select count_if(total > 0) as positive, count(*) as n, regr_count(total, total) as pairs from orders

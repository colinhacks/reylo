select case when vip = 1 then 'vip' else 'regular' end as tier, case when vip = 1 then name end as vip_name, iif(vip = 1, 1, 0) as flag from customers

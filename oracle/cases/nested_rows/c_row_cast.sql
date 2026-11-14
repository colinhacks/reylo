select array(select row(c.body, c.p)::addr from comments c) as addrs

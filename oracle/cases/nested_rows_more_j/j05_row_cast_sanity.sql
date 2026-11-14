select array(select row(c.body, 1)::addr from comments c) as arr

select is_open(column1) as open from posts column1, (values (row(1, $$t$$, null, null, null)::posts)) v

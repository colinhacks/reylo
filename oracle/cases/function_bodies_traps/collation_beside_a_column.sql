select b.id from ci_tab t cross join lateral by_two($$a$$, t.c) b where t.id = 1

select b.id from ci_tab t, lateral by_code(t.c) b where t.id = 1

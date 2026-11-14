select t.d, count(*) as n from ci_tab t where t.d = 'a'::ci_text group by t.d

select array(select c from (select nullif(big, big) as nb, big // 0 as z from comments) c) as arr

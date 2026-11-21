with c as (select least(mask, mask) as digits, mask from flags) select digits, mask from c

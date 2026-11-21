select d.* from (select ifnull(mask, mask) as digits from flags) d

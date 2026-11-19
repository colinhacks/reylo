select coalesce(mask, mask) as c, ifnull(mask, mask) as i, greatest(mask, mask) as g, (least(mask, mask)) as l, mask from flags

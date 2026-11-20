select *, coalesce(mask, mask) as digits, max(mask) over () as most from flags

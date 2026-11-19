select n, any_value(mask) as a, max(mask) as m from flags group by n

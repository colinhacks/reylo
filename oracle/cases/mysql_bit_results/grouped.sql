select n, if(count(*) > 1, max(n), 0) as most, if(n > 0, min(id), 0) as least, if(count(*) > 1, max(enabled), 0) as flag from flags group by n

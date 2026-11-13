select n > 0 as positive, if(count(*) > 1, max(id), 0) as most, if(count(*) > 1, max(enabled), 0) as flag from flags group by positive

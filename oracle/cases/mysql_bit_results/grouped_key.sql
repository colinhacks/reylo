select n, if(count(*) > 1, max(id) + n, 0) as mixed, if(count(*) > 1, concat(n, count(*)), '') as joined from flags group by n

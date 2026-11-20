select id from orders where total > :min and placed < :before and status = :status

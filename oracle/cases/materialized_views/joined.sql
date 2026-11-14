select a.id, l.seats from accounts a join account_list l on l.id = a.id where a.id = :id

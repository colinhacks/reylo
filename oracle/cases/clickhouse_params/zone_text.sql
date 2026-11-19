SELECT toDate(created, {zone:String}) AS day, count() AS n FROM users GROUP BY day

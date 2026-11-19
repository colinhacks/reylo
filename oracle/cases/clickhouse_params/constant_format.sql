SELECT name, formatDateTime(created, {format:String}) AS shown, toStartOfInterval(created, toIntervalMinute({minutes:UInt8})) AS slot FROM users ORDER BY id, name

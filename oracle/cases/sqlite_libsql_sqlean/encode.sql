select encode(data, 'hex') as a, encode(md5(name), 'base64') as b, crypto_encode(md5(nick), 'base32') as c from people

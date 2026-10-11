walk(if type == "object" and has("$ref") then ."$ref" |= sub("^https://developers.strava.com/swagger/"; "./") else . end)

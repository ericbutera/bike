# Select the operations Bike uses, preserving their request and response
# definitions. The overlay supplies only operations absent from the vendor spec.
.paths |= with_entries(select(.key == "/athlete/activities" or .key == "/activities/{id}" or .key == "/activities/{id}/streams"))
| .paths |= with_entries(.value |= with_entries(select(.key == "get")))
|
walk(if type == "object" and has("$ref") then
  ."$ref" |= sub("^https://developers.strava.com/swagger/"; "./vendor/")
else . end)
| .paths += $overlay[0].paths

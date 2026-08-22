"""US state, Canadian provincial/territorial, and Mexican state capitals.

US coordinates: xfront.com states/capitals table. Canada/Mexico: IANA zone1970.tab
where the capital is the tz city, otherwise published city coordinates.
San Jose, CA is included though it is not a capital (Sacramento is).
"""

from __future__ import annotations

# name, lat, lon, IANA tz
US_CAPITALS: tuple[tuple[str, float, float, str], ...] = (
    ("Montgomery", 32.377716, -86.300568, "America/New_York"),
    ("Juneau", 58.301598, -134.420212, "America/Juneau"),
    ("Phoenix", 33.448143, -112.096962, "America/Phoenix"),
    ("Little Rock", 34.746613, -92.288986, "America/Chicago"),
    ("Sacramento", 38.576668, -121.493629, "America/Los_Angeles"),
    ("Denver", 39.739227, -104.984856, "America/Denver"),
    ("Hartford", 41.764046, -72.682198, "America/New_York"),
    ("Dover", 39.157307, -75.519722, "America/New_York"),
    ("Tallahassee", 30.438118, -84.281296, "America/New_York"),
    ("Atlanta", 33.749027, -84.388229, "America/New_York"),
    ("Honolulu", 21.307442, -157.857376, "Pacific/Honolulu"),
    ("Boise", 43.617775, -116.199722, "America/Boise"),
    ("Springfield", 39.798363, -89.654961, "America/Chicago"),
    ("Indianapolis", 39.768623, -86.162643, "America/Indiana/Indianapolis"),
    ("Des Moines", 41.591087, -93.603729, "America/Chicago"),
    ("Topeka", 39.048191, -95.677956, "America/Chicago"),
    ("Frankfort", 38.186722, -84.875374, "America/New_York"),
    ("Baton Rouge", 30.457069, -91.187393, "America/Chicago"),
    ("Augusta", 44.307167, -69.781693, "America/New_York"),
    ("Annapolis", 38.978764, -76.490936, "America/New_York"),
    ("Boston", 42.358162, -71.063698, "America/New_York"),
    ("Lansing", 42.733635, -84.555328, "America/Detroit"),
    ("Saint Paul", 44.955097, -93.102211, "America/Chicago"),
    ("Jackson", 32.303848, -90.182106, "America/Chicago"),
    ("Jefferson City", 38.579201, -92.172935, "America/Chicago"),
    ("Helena", 46.585709, -112.018417, "America/Denver"),
    ("Lincoln", 40.808075, -96.699654, "America/Chicago"),
    ("Carson City", 39.163914, -119.766121, "America/Los_Angeles"),
    ("Concord", 43.206898, -71.537994, "America/New_York"),
    ("Trenton", 40.220596, -74.769913, "America/New_York"),
    ("Santa Fe", 35.68224, -105.939728, "America/Denver"),
    ("Albany", 42.652843, -73.757874, "America/New_York"),
    ("Raleigh", 35.78043, -78.639099, "America/New_York"),
    ("Bismarck", 46.82085, -100.783318, "America/Chicago"),
    ("Columbus", 39.961346, -82.999069, "America/New_York"),
    ("Oklahoma City", 35.492207, -97.503342, "America/Chicago"),
    ("Salem", 44.938461, -123.030403, "America/Los_Angeles"),
    ("Harrisburg", 40.264378, -76.883598, "America/New_York"),
    ("Providence", 41.830914, -71.414963, "America/New_York"),
    ("Columbia", 34.000343, -81.033211, "America/New_York"),
    ("Pierre", 44.367031, -100.346405, "America/Chicago"),
    ("Nashville", 36.16581, -86.784241, "America/Chicago"),
    ("Austin", 30.27467, -97.740349, "America/Chicago"),
    ("Salt Lake City", 40.777477, -111.888237, "America/Denver"),
    ("Montpelier", 44.262436, -72.580536, "America/New_York"),
    ("Richmond", 37.538857, -77.43364, "America/New_York"),
    ("Olympia", 47.035805, -122.905014, "America/Los_Angeles"),
    ("Charleston", 38.336246, -81.612328, "America/New_York"),
    ("Madison", 43.074684, -89.384445, "America/Chicago"),
    ("Cheyenne", 41.140259, -104.820236, "America/Denver"),
    ("Washington, D.C.", 38.9072, -77.0369, "America/New_York"),
)

# Extra US cities requested (not state capitals).
US_EXTRA: tuple[tuple[str, float, float, str], ...] = (
    ("San Jose", 37.3382, -121.8863, "America/Los_Angeles"),
)

CA_CAPITALS: tuple[tuple[str, float, float, str], ...] = (
    ("Edmonton", 53.5461, -113.4938, "America/Edmonton"),
    ("Victoria", 48.4284, -123.3656, "America/Vancouver"),
    ("Winnipeg", 49.8951, -97.1384, "America/Winnipeg"),
    ("Fredericton", 45.9636, -66.6431, "America/Moncton"),
    ("St. John's", 47.5615, -52.7126, "America/St_Johns"),
    ("Halifax", 44.6488, -63.5752, "America/Halifax"),
    ("Toronto", 43.6532, -79.3832, "America/Toronto"),
    ("Charlottetown", 46.2382, -63.1311, "America/Halifax"),
    ("Quebec City", 46.8139, -71.2080, "America/Toronto"),
    ("Regina", 50.4452, -104.6189, "America/Regina"),
    ("Yellowknife", 62.4540, -114.3718, "America/Yellowknife"),
    ("Iqaluit", 63.7467, -68.5170, "America/Iqaluit"),
    ("Whitehorse", 60.7212, -135.0568, "America/Whitehorse"),
    ("Ottawa", 45.4215, -75.6972, "America/Toronto"),
)

MX_CAPITALS: tuple[tuple[str, float, float, str], ...] = (
    ("Aguascalientes", 21.8853, -102.2916, "America/Mexico_City"),
    ("Mexicali", 32.6245, -115.4523, "America/Tijuana"),
    ("La Paz", 24.1426, -110.3128, "America/Mazatlan"),
    ("Campeche", 19.8301, -90.5349, "America/Merida"),
    ("Tuxtla Gutierrez", 16.7516, -93.1031, "America/Mexico_City"),
    ("Chihuahua", 28.6353, -106.0889, "America/Chihuahua"),
    ("Saltillo", 25.4232, -101.0053, "America/Monterrey"),
    ("Colima", 19.2433, -103.7250, "America/Mexico_City"),
    ("Durango", 24.0277, -104.6532, "America/Monterrey"),
    ("Guanajuato", 21.0190, -101.2574, "America/Mexico_City"),
    ("Chilpancingo", 17.5515, -99.5006, "America/Mexico_City"),
    ("Pachuca", 20.1011, -98.7591, "America/Mexico_City"),
    ("Guadalajara", 20.6597, -103.3496, "America/Mexico_City"),
    ("Toluca", 19.2826, -99.6557, "America/Mexico_City"),
    ("Mexico City", 19.4326, -99.1332, "America/Mexico_City"),
    ("Morelia", 19.7060, -101.1950, "America/Mexico_City"),
    ("Cuernavaca", 18.9242, -99.2216, "America/Mexico_City"),
    ("Tepic", 21.5041, -104.8946, "America/Mazatlan"),
    ("Monterrey", 25.6866, -100.3161, "America/Monterrey"),
    ("Oaxaca", 17.0732, -96.7266, "America/Mexico_City"),
    ("Puebla", 19.0414, -98.2063, "America/Mexico_City"),
    ("Queretaro", 20.5888, -100.3899, "America/Mexico_City"),
    ("Chetumal", 18.5002, -88.2961, "America/Cancun"),
    ("San Luis Potosi", 22.1565, -100.9855, "America/Mexico_City"),
    ("Culiacan", 24.8091, -107.3940, "America/Mazatlan"),
    ("Hermosillo", 29.0729, -110.9559, "America/Hermosillo"),
    ("Villahermosa", 17.9892, -92.9281, "America/Mexico_City"),
    ("Ciudad Victoria", 23.7362, -99.1411, "America/Monterrey"),
    ("Tlaxcala", 19.3182, -98.2375, "America/Mexico_City"),
    ("Xalapa", 19.5438, -96.9102, "America/Mexico_City"),
    ("Merida", 20.9674, -89.5926, "America/Merida"),
    ("Zacatecas", 22.7709, -102.5832, "America/Mexico_City"),
)

ALIASES: dict[str, str] = {
    "washington dc": "Washington, D.C.",
    "washington, dc": "Washington, D.C.",
    "washington d.c.": "Washington, D.C.",
    "dc": "Washington, D.C.",
    "d.c.": "Washington, D.C.",
    "san jose, ca": "San Jose",
    "san jose ca": "San Jose",
    "san jose, california": "San Jose",
    "san jose california": "San Jose",
    "st paul": "Saint Paul",
    "st. paul": "Saint Paul",
    "quebec": "Quebec City",
    "quebec city": "Quebec City",
    "ciudad de mexico": "Mexico City",
    "cdmx": "Mexico City",
    "tuxtla gutierrez": "Tuxtla Gutierrez",
    "san luis potosi": "San Luis Potosi",
    "st johns": "St. John's",
    "st. johns": "St. John's",
}


def all_capitals() -> tuple[tuple[str, float, float, str], ...]:
    return US_CAPITALS + US_EXTRA + CA_CAPITALS + MX_CAPITALS

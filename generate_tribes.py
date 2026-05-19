import json

names = [
    # Native American (North, Meso, South)
    "Apache", "Comanche", "Cherokee", "Navajo", "Maya", "Aztec", "Olmec", "Zapotec", 
    "Toltec", "Mixtec", "Totonac", "Purépecha", "Huastec", "Teotihuacan", "Tlaxcaltec", 
    "Taino", "Carib", "Arawak", "Inca", "Muisca", "Guarani", "Mapuche", "Iroquois", 
    "Huron", "Algonquin", "Cree", "Ojibwe", "Micmac", "Shawnee", "Miami", "Illini", 
    "Kickapoo", "Potawatomi", "Menominee", "Ho-Chunk", "Sauk", "Fox", "Iowa", "Otoe", 
    "Missouri", "Omaha", "Ponca", "Osage", "Kansa", "Quapaw", "Mandan", "Hidatsa", 
    "Arikara", "Pawnee", "Wichita", "Caddo", "Kitsai", "Tonkawa", "Karankawa", "Coahuiltecan",
    "Chichimeca", "Yaqui", "Tarahumara", "Mayo", "Cora", "Huichol", "Tepehuan", "Pima",
    "Papago", "Hopi", "Zuni", "Acoma", "Laguna", "Isleta", "Sandia", "Jemez", "Zia",
    "Santa Ana", "San Felipe", "Santo Domingo", "Cochiti", "Tesuque", "Pojoaque", "Nambe",
    "San Ildefonso", "Santa Clara", "San Juan", "Picuris", "Taos", "Ute", "Paiute",
    "Shoshone", "Bannock", "Goshute", "Washoe", "Miwok", "Yokuts", "Chumash", "Salinan",
    "Costanoan", "Esselen", "Wintun", "Maidu", "Yana", "Achumawi", "Atsugewi", "Modoc",
    "Klamath", "Shasta", "Karuk", "Yurok", "Hupa", "Tolowa", "Wiyot", "Mattole",
    "Sinkyone", "Wailaki", "Kato", "Pomo", "Yuki", "Wappo", "Lake Miwok", "Coast Miwok",
    # European / Eurasian Antiquity
    "Goths", "Vandals", "Franks", "Saxons", "Angles", "Jutes", "Celts", "Picts", 
    "Gaels", "Britons", "Iceni", "Brigantes", "Suebi", "Alemanni", "Burgundians", 
    "Lombards", "Thuringii", "Frisians", "Rugii", "Heruli", "Gepids", "Sciri", "Alans", 
    "Huns", "Avars", "Magyars", "Bulgars", "Khazars", "Pechenegs", "Cumans", "Kipchaks", 
    "Slavs", "Rus", "Polans", "Drevlians", "Radimichs", "Vyatichs", "Krivichs", 
    "Ilmen Slavs", "Dregovichs", "Severians", "Dulebes", "White Croats", "Tivertsi", 
    "Ulichs", "Volhynians", "Buzhans", "Drevlyans", "Sclaveni", "Antes", "Venethi",
    "Aesti", "Venedi", "Fenni", "Sitones", "Suiones", "Gautigoths", "Ostrogoths",
    "Visigoths", "Teruingi", "Greuthungi", "Taifals", "Bastarnae", "Peucini", "Costoboci",
    "Carpi", "Dacians", "Getae", "Thracians", "Odrysians", "Triballi", "Moesi", "Crobyzi",
    "Scordisci", "Serdi", "Maedi", "Bessi", "Dii", "Satrae", "Odomanti", "Edoni",
    "Bisaltae", "Crestones", "Mygdones", "Sithones", "Agrianes", "Paeonians", "Pelagonians",
    "Lyncestae", "Orestae", "Elimiotae", "Tymphaei", "Parauaei", "Chaonians", "Molossians",
    "Thesprotians", "Cassopaeans", "Athamanes", "Aethices", "Talares", "Oetaeans", "Aenianes",
    "Malians", "Dolopes", "Magnetes", "Perrhaebi", "Lapiths", "Centaurs", "Myrmidons",
    # Africa
    "Zulu", "Xhosa", "Maasai", "Yoruba", "Igbo", "Hausa", "Oromo", "Amhara", "Somali", 
    "Akan", "Ashanti", "Fon", "Ewe", "Bakongo", "Luba", "Lunda", "Shona", "Ndebele", 
    "San", "Khoikhoi", "Tuareg", "Berber", "Fulani", "Mandinka", "Songhai", "Wolof", 
    "Serer", "Dogon", "Mossi", "Dinka", "Nuer", "Shilluk", "Baganda", "Kikuyu",
    "Luo", "Kamba", "Kalenjin", "Turkana", "Samburu", "Rendille", "Boran", "Gabra",
    "Somali", "Afar", "Tigray", "Tigre", "Bilen", "Saho", "Kunama", "Nara", "Hedareb",
    "Rashaida", "Beja", "Nubians", "Fur", "Zaghawa", "Masalit", "Tama", "Daju",
    "Berti", "Bidyogo", "Baga", "Nalu", "Landuma", "Susu", "Jalonke", "Bambara",
    "Malinke", "Soninke", "Bozo", "Somono", "Khassonke", "Dialonke", "Kuranko", "Loko",
    "Mende", "Temne", "Bullom", "Sherbro", "Krim", "Vai", "Gola", "Kissi", "Kpelle",
    "Loma", "Gbandi", "Mano", "Dan", "Wee", "Krahn", "Grebo", "Kru", "Bassa",
    # Asia
    "Akkadians", "Sumerians", "Babylonians", "Assyrians", "Hittites", "Elamites", 
    "Medes", "Persians", "Parthians", "Sassanids", "Scythians", "Sarmatians", "Massagetae", 
    "Yuezhi", "Xiongnu", "Xianbei", "Rouran", "Göktürks", "Uyghurs", "Khitans", "Jurchens", 
    "Mongols", "Tatars", "Manchus", "Yamato", "Emishi", "Ainu", "Yayoi", "Jomon", 
    "Buyeo", "Goguryeo", "Baekje", "Silla", "Gaya", "Balhae", "Chola", "Chera", 
    "Pandya", "Maurya", "Gupta", "Kushan", "Rajput", "Maratha", "Sikh", "Mughal", 
    "Safavid", "Ottomans", "Seljuks", "Ghaznavids", "Ghurids", "Khwarezmians", "Timurids",
    "Qajars", "Afsharids", "Zands", "Hotakis", "Durranis", "Barakzais", "Samanids",
    "Tahirids", "Saffarids", "Buyids", "Ziyarids", "Qarakhanids", "Qara Khitai", "Chagatais",
    "Ilkhanids", "Golden Horde", "White Horde", "Blue Horde", "Nogais", "Uzbeks", "Kazakhs",
    "Kyrgyz", "Turkmens", "Tajiks", "Pashtuns", "Baloch", "Brahui", "Sindhis", "Punjabis",
    "Kashmiris", "Dogras", "Paharis", "Garhwalis", "Kumaonis", "Nepalis", "Bhutias", "Lepchas",
    "Gorkhas", "Apatani", "Naga", "Mizo", "Garo", "Khasi", "Jaintia", "Bodo", "Dimasa", "Karbi",
    "Kuki", "Meitei", "Tripuri", "Reang", "Chakma", "Mog", "Santhal", "Munda", "Ho", "Bhumij",
    "Kharia", "Oraon", "Gond", "Bhils", "Kolis", "Warlis", "Bhilala", "Barela", "Patelia",
    "Rathwa", "Naikda", "Dhanka", "Gamit", "Chaudhari", "Vasava", "Kotwalia", "Kathodi",
    "Siddi", "Rabari", "Bharwad", "Maldhari", "Ahir", "Jat", "Gujjar", "Rajput", "Brahmin",
    "Yadava", "Kurmi", "Koeri", "Kushwaha", "Maurya", "Saini", "Mali", "Kachhi", "Lodh",
    "Guanches", "Canarians", "Maccabees", "Hasmoneans", "Idumeans", "Nabataeans", "Edomites",
    "Ammonites", "Moabites", "Midianites", "Philistines", "Phoenicians", "Canaanites", "Aramaeans",
    "Chaldaeans", "Kassites", "Hurrians", "Urartians", "Phrygians", "Lydians", "Carians", "Lycians",
    "Pamphylians", "Pisidians", "Isaurians", "Cilicians", "Cappadocians", "Pontians", "Bithynians",
    # Oceania / Pacific
    "Maori", "Samoans", "Tongans", "Fijians", "Hawaiians", "Tahitians", "Marquesans", 
    "Rapa Nui", "Chamorro", "Palauan", "Yapese", "Chuukese", "Pohnpeian", "Kosraean", 
    "Marshallese", "Kiribati", "Tuvaluan", "Tokelauan", "Niuean", "Cook Islanders",
    "Wallisians", "Futunans", "Rotumans", "Vanuatuans", "Solomon Islanders", "Papuans",
    "Melanesians", "Micronesians", "Polynesians", "Austronesians", "Aborigines", "Torres Strait Islanders",
    "Tiwi", "Palawa", "Nunga", "Koori", "Murri", "Noongar", "Yamatji", "Wonghi",
    "Anangu", "Pintupi", "Pitjantjatjara", "Yankunytjatjara", "Luritja", "Ngaanyatjarra", "Arrernte",
    "Warlpiri", "Warumungu", "Kaytetye", "Alyawarre", "Anmatyerre", "Eastern Arrernte", "Western Arrernte",
    # Pre-Columbian and South American
    "Chavin", "Paracas", "Nazca", "Moche", "Tiwanaku", "Wari", "Chimu", "Inca",
    "Tairona", "Quimbaya", "Zenú", "Calima", "Tierradentro", "San Agustín", "Tumaco",
    "Tolita", "Jama-Coaque", "Bahía", "Guangala", "Manteño", "Huancavilca", "Milagro",
    "Quevedo", "Cañari", "Puruhá", "Panzaleo", "Caranqui", "Pasto", "Quillacinga",
    "Sindagua", "Noanamá", "Emberá", "Wounaan", "Kuna", "Ngäbe", "Buglé",
    "Bribri", "Cabécar", "Teribe", "Boruca", "Chorotega", "Nicarao", "Subtiaba",
    "Matagalpa", "Lenca", "Jicaque", "Paya", "Sumu", "Miskito", "Rama",
    "Maya", "Pipil", "Xinca", "Pokomam", "Pokomchi", "Kekchi", "Uspantec",
    "Ixil", "Aguacatec", "Mam", "Tectitec", "Sipacapense", "Tzutuil", "Cakchiquel",
    "Quiche", "Rabinal", "Sacapultec", "Chol", "Chontal", "Tzeltal", "Tzotzil",
    "Tojolabal", "Chuj", "Kanjobal", "Jacaltec", "Motozintlec", "Tuzantec", "Mocho",
    # Additional ancient/mythical/lost tribes
    "Atlanteans", "Lemurians", "Muvians", "Hyperboreans", "Amazons", "Gargareans", "Arimaspi",
    "Issedones", "Agathyrsi", "Neuri", "Androphagi", "Melanchlaeni", "Budini", "Geloni",
    "Thyssagetae", "Iyrcae", "Argippaei", "Pygmies", "Blemmyae", "Sciapods", "Cynocephali",
    "Panotii", "Astomi", "Amyctyrae", "Monocoli", "Artibatirae", "Catenates", "Caturiges",
    "Ceutrones", "Medulli", "Graioceli", "Segusini", "Taurini", "Salassi", "Lepontii",
    "Vindelici", "Raeti", "Camunni", "Euganei", "Veneti", "Histri", "Liburni",
    "Iapydes", "Dalmatiae", "Daorsi", "Ardiaei", "Autariatae", "Dardani", "Paeones",
    "Maedi", "Dentheletae", "Bessi", "Dii", "Satrae", "Odomanti", "Edoni", "Bisaltae"
]

names = list(set(names)) # Remove duplicates just in case
print(f"Generated {len(names)} unique names.")

rust_code = f"""// Auto-generated fallback list of historical tribes and nations
pub const FALLBACK_TRIBES: &[&str] = &[
"""
for name in names:
    rust_code += f'    "{name}",\n'
rust_code += "];\n"

with open("sow-core/src/tribes.rs", "w") as f:
    f.write(rust_code)

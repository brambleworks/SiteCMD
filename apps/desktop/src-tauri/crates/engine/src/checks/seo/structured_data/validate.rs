//! Consumer-specific JSON-LD property checks, not full Schema.org validation.

use serde_json::Value;
use std::collections::BTreeSet;

type Obj = serde_json::Map<String, Value>;

// Explicit descendants avoid suffix-based false positives and false negatives.
const LOCAL_BUSINESS_TYPES: &[&str] = &[
    "AccountingService",
    "AdultEntertainment",
    "AmusementPark",
    "AnimalShelter",
    "ArchiveOrganization",
    "ArtGallery",
    "Attorney",
    "AutoBodyShop",
    "AutoDealer",
    "AutoPartsStore",
    "AutoRental",
    "AutoRepair",
    "AutoWash",
    "AutomatedTeller",
    "AutomotiveBusiness",
    "Bakery",
    "BankOrCreditUnion",
    "BarOrPub",
    "BeautySalon",
    "BedAndBreakfast",
    "BikeStore",
    "BookStore",
    "BowlingAlley",
    "Brewery",
    "CafeOrCoffeeShop",
    "Campground",
    "Casino",
    "ChildCare",
    "ClothingStore",
    "ComedyClub",
    "ComputerStore",
    "ConvenienceStore",
    "CovidTestingFacility",
    "DaySpa",
    "Dentist",
    "DepartmentStore",
    "Distillery",
    "DryCleaningOrLaundry",
    "Electrician",
    "ElectronicsStore",
    "EmergencyService",
    "EmploymentAgency",
    "EntertainmentBusiness",
    "ExerciseGym",
    "FastFoodRestaurant",
    "FinancialService",
    "FireStation",
    "Florist",
    "FoodEstablishment",
    "FurnitureStore",
    "GardenStore",
    "GasStation",
    "GeneralContractor",
    "GolfCourse",
    "GovernmentOffice",
    "GroceryStore",
    "HVACBusiness",
    "HairSalon",
    "HardwareStore",
    "HealthAndBeautyBusiness",
    "HealthClub",
    "HobbyShop",
    "HomeAndConstructionBusiness",
    "HomeGoodsStore",
    "Hospital",
    "Hostel",
    "Hotel",
    "HousePainter",
    "IceCreamShop",
    "IndividualPhysician",
    "InsuranceAgency",
    "InternetCafe",
    "JewelryStore",
    "LegalService",
    "Library",
    "LiquorStore",
    "LocalBusiness",
    "Locksmith",
    "LodgingBusiness",
    "MedicalBusiness",
    "MedicalClinic",
    "MensClothingStore",
    "MobilePhoneStore",
    "Motel",
    "MotorcycleDealer",
    "MotorcycleRepair",
    "MovieRentalStore",
    "MovieTheater",
    "MovingCompany",
    "MusicStore",
    "NailSalon",
    "NightClub",
    "Notary",
    "OfficeEquipmentStore",
    "Optician",
    "OutletStore",
    "PawnShop",
    "PetStore",
    "Pharmacy",
    "Physician",
    "PhysiciansOffice",
    "Plumber",
    "PoliceStation",
    "PostOffice",
    "ProfessionalService",
    "PublicSwimmingPool",
    "RadioStation",
    "RealEstateAgent",
    "RecyclingCenter",
    "Resort",
    "Restaurant",
    "RoofingContractor",
    "SelfStorage",
    "ShoeStore",
    "ShoppingCenter",
    "SkiResort",
    "SportingGoodsStore",
    "SportsActivityLocation",
    "SportsClub",
    "StadiumOrArena",
    "Store",
    "TattooParlor",
    "TelevisionStation",
    "TennisComplex",
    "TireShop",
    "TouristInformationCenter",
    "ToyStore",
    "TravelAgency",
    "VacationRental",
    "WholesaleStore",
    "Winery",
];

// Organization descendants that are not already LocalBusiness descendants.
// This is used only for the bounded identity-presence signal, not to assert
// that the node truthfully represents the publisher.
const NON_LOCAL_ORGANIZATION_TYPES: &[&str] = &[
    "Airline",
    "CollegeOrUniversity",
    "Consortium",
    "Cooperative",
    "Corporation",
    "DanceGroup",
    "DiagnosticLab",
    "EducationalOrganization",
    "ElementarySchool",
    "FundingAgency",
    "FundingScheme",
    "GovernmentOrganization",
    "HighSchool",
    "LibrarySystem",
    "MedicalOrganization",
    "MiddleSchool",
    "MusicGroup",
    "NGO",
    "NewsMediaOrganization",
    "OnlineBusiness",
    "OnlineMarketplace",
    "OnlineStore",
    "Organization",
    "PerformingGroup",
    "PoliticalParty",
    "Preschool",
    "Project",
    "ResearchOrganization",
    "ResearchProject",
    "School",
    "SearchRescueOrganization",
    "SportsOrganization",
    "SportsTeam",
    "TheaterGroup",
    "VeterinaryCare",
    "WorkersUnion",
];

pub(super) fn is_local_business_type(type_name: &str) -> bool {
    LOCAL_BUSINESS_TYPES
        .iter()
        .any(|known| known.eq_ignore_ascii_case(type_name))
}

pub(super) fn is_identity_type(type_name: &str) -> bool {
    type_name.eq_ignore_ascii_case("Person")
        || is_local_business_type(type_name)
        || NON_LOCAL_ORGANIZATION_TYPES
            .iter()
            .any(|known| known.eq_ignore_ascii_case(type_name))
}

/// What validation concluded about a single JSON-LD node.
pub enum NodeValidation {
    /// Node had a recognized @type; property gaps listed (possibly empty).
    Recognized(TypeFindings),
    /// Node declared @type values, none of which this module validates.
    Unrecognized(Vec<String>),
    /// Node had no usable @type (for example a bare @context wrapper).
    Untyped,
}

/// Property gaps found on a node of a recognized type.
pub struct TypeFindings {
    pub type_name: String,
    pub missing_required: BTreeSet<String>,
    pub missing_recommended: BTreeSet<String>,
}

impl TypeFindings {
    fn new(type_name: &str) -> Self {
        Self {
            type_name: type_name.to_string(),
            missing_required: BTreeSet::new(),
            missing_recommended: BTreeSet::new(),
        }
    }

    fn require(&mut self, obj: &Obj, key: &str) {
        if !present(obj, key) {
            self.missing_required.insert(key.to_string());
        }
    }

    fn recommend(&mut self, obj: &Obj, key: &str) {
        if !present(obj, key) {
            self.missing_recommended.insert(key.to_string());
        }
    }

    pub fn is_complete(&self) -> bool {
        self.missing_required.is_empty() && self.missing_recommended.is_empty()
    }
}

/// Validate one node. @type may be a string or an array; the first recognized
/// entry decides which rule set applies.
pub fn validate_node(node: &Value) -> NodeValidation {
    let Some(obj) = node.as_object() else {
        return NodeValidation::Untyped;
    };
    let names = type_names(obj);
    if names.is_empty() {
        return NodeValidation::Untyped;
    }
    for name in &names {
        if let Some(findings) = validate_recognized(name, obj) {
            return NodeValidation::Recognized(findings);
        }
    }
    NodeValidation::Unrecognized(names)
}

fn type_names(obj: &Obj) -> Vec<String> {
    match obj.get("@type") {
        Some(Value::String(name)) => vec![name.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

fn validate_recognized(type_name: &str, obj: &Obj) -> Option<TypeFindings> {
    let lower = type_name.to_ascii_lowercase();
    Some(match lower.as_str() {
        "article" | "blogposting" | "newsarticle" => article(type_name, obj),
        "product" => product(obj),
        "breadcrumblist" => breadcrumb_list(obj),
        "website" => web_site(obj),
        "organization" => organization(obj),
        _ if is_local_business_type(type_name) => local_business(type_name, obj),
        _ => return None,
    })
}

/// A property counts as present when it exists and is not null, an empty
/// string, an empty array, or an empty object.
fn present(obj: &Obj, key: &str) -> bool {
    match obj.get(key) {
        None | Some(Value::Null) => false,
        Some(Value::String(text)) => !text.trim().is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(inner)) => !inner.is_empty(),
        Some(_) => true,
    }
}

fn article(type_name: &str, obj: &Obj) -> TypeFindings {
    // Article has recommended, not required, properties; headline absence is
    // therefore advisory.
    let mut findings = TypeFindings::new(type_name);
    findings.recommend(obj, "headline");
    findings.recommend(obj, "author");
    findings.recommend(obj, "datePublished");
    findings.recommend(obj, "image");
    findings
}

fn product(obj: &Obj) -> TypeFindings {
    let mut findings = TypeFindings::new("Product");
    findings.require(obj, "name");
    findings.recommend(obj, "image");

    // Google's product-snippet profile requires at least one of these three
    // branches. Merely having an `offers` key is insufficient when it is
    // empty or not an Offer-like object.
    let offer_objects: Vec<(&Obj, String)> = match obj.get("offers") {
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                value
                    .as_object()
                    .map(|offer| (offer, format!("offers[{index}]")))
            })
            .collect(),
        Some(Value::Object(offer)) if !offer.is_empty() => vec![(offer, "offers".to_string())],
        _ => Vec::new(),
    };
    if !present(obj, "review") && !present(obj, "aggregateRating") && offer_objects.is_empty() {
        findings
            .missing_required
            .insert("review, aggregateRating, or offers".to_string());
    }

    for (offer, path) in offer_objects {
        if has_type(offer, "AggregateOffer") {
            if !present(offer, "lowPrice") {
                findings.missing_required.insert(format!("{path}.lowPrice"));
            }
            if !offer_has_field(offer, "priceCurrency") {
                findings
                    .missing_required
                    .insert(format!("{path}.priceCurrency"));
            }
        } else {
            if !offer_has_price(offer) {
                findings.missing_required.insert(format!("{path}.price"));
            }
            if !offer_has_field(offer, "priceCurrency") {
                findings
                    .missing_recommended
                    .insert(format!("{path}.priceCurrency"));
            }
        }
    }
    findings
}

fn has_type(obj: &Obj, expected: &str) -> bool {
    match obj.get("@type") {
        Some(Value::String(value)) => value.eq_ignore_ascii_case(expected),
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .any(|value| value.eq_ignore_ascii_case(expected)),
        _ => false,
    }
}

/// Offer price can live at `price`, `lowPrice` (AggregateOffer), or inside
/// `priceSpecification`.
fn offer_has_price(offer: &Obj) -> bool {
    present(offer, "lowPrice") || offer_has_field(offer, "price")
}

fn offer_has_field(offer: &Obj, key: &str) -> bool {
    if present(offer, key) {
        return true;
    }
    match offer.get("priceSpecification") {
        Some(Value::Object(spec)) => present(spec, key),
        Some(Value::Array(specs)) => specs
            .iter()
            .filter_map(Value::as_object)
            .any(|spec| present(spec, key)),
        _ => false,
    }
}

fn breadcrumb_list(obj: &Obj) -> TypeFindings {
    let mut findings = TypeFindings::new("BreadcrumbList");
    match obj.get("itemListElement") {
        Some(Value::Array(items)) if !items.is_empty() => {
            if items.len() < 2 {
                findings
                    .missing_required
                    .insert("itemListElement (at least two entries)".to_string());
            }
            let last_index = items.len() - 1;
            for (index, value) in items.iter().enumerate() {
                let Some(item) = value.as_object() else {
                    findings
                        .missing_required
                        .insert(format!("itemListElement[{index}] (ListItem object)"));
                    continue;
                };
                if !present(item, "position") {
                    findings
                        .missing_required
                        .insert(format!("itemListElement[{index}].position"));
                }
                // Google permits the final breadcrumb to omit `item` and use
                // the containing page URL. Earlier entries still need it.
                if index != last_index && !present(item, "item") {
                    findings
                        .missing_required
                        .insert(format!("itemListElement[{index}].item"));
                }
                // A top-level name is required unless an object-valued `item`
                // carries its own name. A URL string alone is not a name.
                let nested_item_name = item
                    .get("item")
                    .and_then(Value::as_object)
                    .is_some_and(|nested| present(nested, "name"));
                if !present(item, "name") && !nested_item_name {
                    findings
                        .missing_required
                        .insert(format!("itemListElement[{index}].name"));
                }
            }
        }
        _ => {
            findings
                .missing_required
                .insert("itemListElement".to_string());
        }
    }
    findings
}

fn local_business(type_name: &str, obj: &Obj) -> TypeFindings {
    let mut findings = TypeFindings::new(type_name);
    findings.require(obj, "name");
    findings.require(obj, "address");
    findings
}

fn web_site(obj: &Obj) -> TypeFindings {
    let mut findings = TypeFindings::new("WebSite");
    findings.require(obj, "name");
    findings.require(obj, "url");
    findings
}

/// Google's Organization profile has no universally required properties;
/// these are low-confidence completeness recommendations only.
fn organization(obj: &Obj) -> TypeFindings {
    let mut findings = TypeFindings::new("Organization");
    if !present(obj, "name") && !present(obj, "alternateName") {
        findings
            .missing_recommended
            .insert("name (or alternateName)".to_string());
    }
    findings.recommend(obj, "url");
    findings.recommend(obj, "logo");
    findings
}

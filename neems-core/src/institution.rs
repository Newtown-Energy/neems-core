use rand::rng;
use rand::prelude::IndexedRandom;


pub fn random_energy_company_names(count: usize) -> Vec<&'static str> {
    let names = vec![
        "Voltara Energy", "Dynamis Power", "Nextera Solutions", "Energex Systems",
        "OmniVolt Energy", "PowerSphere", "TerraVolt", "Aegis Energy",
        "Lumina Power", "CoreGrid Energy", "StorVolt Technologies", "PowerCell Dynamics",
        "EverCharge Storage", "VoltCache", "NanoGrid Batteries", "EnerStore Solutions",
        "FluxCap Energy", "QuantumCell Storage", "PowerBank Energy", "IonCore Storage",
        "SunForge Energy", "HeliosRay Solar", "SolaraGrid", "Photonix Solar",
        "SunCore Renewables", "AuroraSolar Tech", "Lumos Solar", "Solaris Dynamics",
        "BrightVolt Solar", "SunTerra Power", "AeroVolt Wind", "GaleForce Energy",
        "WindStrider Power", "TurbineX Solutions", "Breeza Energy", "SkyBlade Wind",
        "Vortex Power", "ZephyrWind Tech", "Cyclone Energy", "WindHarbor Systems",
        "TitanGrid Power", "PrimeVolt Plants", "EcoGen Energy", "TerraFirma Power",
        "GridForge Solutions", "OmniPlant Energy", "NovaCore Power", "Apex Energy Plants",
        "VoltForge Utilities", "PowerHaven Energy", "AtomForge Energy", "Neutron Power Co.",
        "FissionX Solutions", "NucleoVolt", "QuantumFission Energy", "AtomGrid Systems",
        "CoreNova Power", "Isotope Dynamics", "ReactorX Energy", "Atomic Horizon",
        "Fusionis Energy", "StellarCore Fusion", "PlasmaVolt Tech", "Helion Dynamics",
        "SunFusion Power", "QuantumPlasma", "Ignis Fusion", "NovaFusion Systems",
        "ThermoCore Energy", "FusionForge", "HybridVolt Energy", "NextGen Power Co.",
        "SynthEnergy Solutions", "EcoVolt Innovations", "HyperGrid Energy", "GreenCore Dynamics",
        "Infinity Power", "OptiVolt Systems", "Aether Energy", "NeoVolt Technologies",
        "VerdePower", "EcoSphere Energy", "SustainaVolt", "GreenPulse Energy",
        "TerraWatt Solutions", "RenewaCore", "EarthVolt Power", "PureEnergy Dynamics",
        "BioVolt Renewables", "CleanGrid Tech", "QuantumVolt", "Hyperion Energy",
        "Nebula Power", "Astralis Energy", "CyberVolt", "NanoVolt Tech",
        "Pulsar Energy", "Omega Power Systems", "Xenon Energy", "Zenith Power Co.",
    ];
    let mut rng = rng();
    let selected: Vec<_> = names.choose_multiple(&mut rng, count).copied().collect();
    selected
}


pub use crate::orm::institution::{get_institution_by_name, insert_institution};



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_energy_company_names() {
        let names = random_energy_company_names(5);
        assert_eq!(names.len(), 5);
        
        let all_names = random_energy_company_names(10);
        assert_eq!(all_names.len(), 10);
        
        assert!(names.iter().all(|name| !name.is_empty()));
    }
}

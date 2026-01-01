use crate::artifact::Artifact;
use crate::project::{Dependency, DependencyManagement, PomParserError, Project};
use crate::{ArtifactId, Classifier, GroupId, Version};
use std::collections::HashMap;
use std::io::{BufReader, Read, Seek};
use xml::EventReader;
use xml::reader::XmlEvent;

pub struct PomParser {}

impl PomParser {
    pub fn parse<R: Read + Seek>(input: R) -> Result<Project, PomParserError> {
        let buffer = BufReader::new(input);
        let mut parser = EventReader::new(buffer);
        Self::parse_project(&mut parser)
    }

    fn parse_project<R: Read + Seek>(
        parser: &mut EventReader<BufReader<R>>,
    ) -> Result<Project, PomParserError> {
        let mut state = ArtifactState::default();
        let mut parent = None;
        let mut dependencies = Vec::new();
        let mut dependency_management = DependencyManagement::default();
        let mut properties = HashMap::default();
        loop {
            let event = &parser.next()?;
            match event {
                XmlEvent::StartElement { name, .. } if name.local_name == "groupId" => {
                    let id = Self::string_element(parser)?;
                    state.group_id = Some(GroupId::from(id));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "artifactId" => {
                    let id = Self::string_element(parser)?;
                    state.artifact_id = Some(ArtifactId::from(id));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "version" => {
                    let id = Self::string_element(parser)?;
                    state.version = Some(Version::from(id));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "packaging" => {
                    let id = Self::string_element(parser)?;
                    state.extension = Some(id);
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "classifier" => {
                    let id = Self::string_element(parser)?;
                    state.classifier = Some(Classifier::from(id));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "parent" => {
                    let p = Self::parse_parent(parser)?;
                    parent = Some(p);
                }
                XmlEvent::StartElement { name, .. }
                    if name.local_name == "dependencyManagement" =>
                {
                    dependency_management = Self::parse_dependency_management(parser)?;
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "dependencies" => {
                    dependencies = Self::parse_dependencies(parser)?;
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "properties" => {
                    properties = Self::parse_properties(parser)?;
                }

                XmlEvent::EndDocument => {
                    return Ok(Project {
                        artifact: state.to_artifact()?,
                        parent,
                        dependency_management,
                        dependencies,
                        properties,
                    });
                }
                _ => (),
            }
        }
    }

    fn parse_parent<R: Read + Seek>(
        parser: &mut EventReader<BufReader<R>>,
    ) -> Result<Artifact, PomParserError> {
        let mut state = ArtifactState::default();
        loop {
            let event = &parser.next()?;
            match event {
                XmlEvent::StartElement { name, .. } if name.local_name == "groupId" => {
                    let id = Self::string_element(parser)?;
                    state.group_id = Some(GroupId::from(id));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "artifactId" => {
                    let id = Self::string_element(parser)?;
                    state.artifact_id = Some(ArtifactId::from(id));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "version" => {
                    let id = Self::string_element(parser)?;
                    state.version = Some(Version::from(id));
                }
                XmlEvent::EndElement { name, .. } if name.local_name == "parent" => {
                    return state.to_artifact();
                }
                _ => (),
            }
        }
    }

    fn parse_dependency<R: Read + Seek>(
        parser: &mut EventReader<BufReader<R>>,
    ) -> Result<Dependency, PomParserError> {
        let mut state = ArtifactState::default();
        let mut scope = Option::default();
        loop {
            let event = &parser.next()?;
            match event {
                XmlEvent::StartElement { name, .. } if name.local_name == "groupId" => {
                    let id = Self::string_element(parser)?;
                    state.group_id = Some(GroupId::from(id));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "artifactId" => {
                    let id = Self::string_element(parser)?;
                    state.artifact_id = Some(ArtifactId::from(id));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "version" => {
                    let id = Self::string_element(parser)?;
                    state.version = Some(Version::from(id));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "type" => {
                    let id = Self::string_element(parser)?;
                    state.extension = Some(id);
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "classifier" => {
                    let id = Self::string_element(parser)?;
                    state.classifier = Some(Classifier::from(id));
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "scope" => {
                    let id = Self::string_element(parser)?;
                    scope = Some(id);
                }
                XmlEvent::EndElement { name, .. } if name.local_name == "dependency" => {
                    return Ok(Dependency {
                        artifact: state.to_artifact()?,
                        scope: scope.clone(),
                    });
                }
                _ => (),
            }
        }
    }

    fn string_element<R: Read + Seek>(
        parser: &mut EventReader<BufReader<R>>,
    ) -> Result<String, PomParserError> {
        let out = match &parser.next()? {
            XmlEvent::Characters(chars) => Ok(chars.to_owned()),
            e => Err(PomParserError::Unexpected(format!("{:?}", e))),
        }?;
        parser.next()?;
        Ok(out)
    }

    fn parse_dependency_management<R: Read + Seek>(
        parser: &mut EventReader<BufReader<R>>,
    ) -> Result<DependencyManagement, PomParserError> {
        let mut state = DependencyManagement::default();
        loop {
            let event = &parser.next()?;
            match event {
                XmlEvent::StartElement { name, .. } if name.local_name == "dependencies" => {
                    state.dependencies = Self::parse_dependencies(parser)?;
                }
                XmlEvent::EndElement { name, .. } if name.local_name == "dependencyManagement" => {
                    return Ok(state);
                }
                _ => (),
            }
        }
    }

    fn parse_dependencies<R: Read + Seek>(
        parser: &mut EventReader<BufReader<R>>,
    ) -> Result<Vec<Dependency>, PomParserError> {
        let mut state = vec![];
        loop {
            let event = &parser.next()?;
            match event {
                XmlEvent::EndElement { name, .. } if name.local_name == "dependencies" => {
                    return Ok(state);
                }
                XmlEvent::StartElement { name, .. } if name.local_name == "dependency" => {
                    let artifact = Self::parse_dependency(parser)?;
                    state.push(artifact);
                }
                _ => (),
            }
        }
    }

    fn parse_properties<R: Read + Seek>(
        parser: &mut EventReader<BufReader<R>>,
    ) -> Result<HashMap<String, String>, PomParserError> {
        let mut state = HashMap::new();
        loop {
            let event = &parser.next()?;
            match event {
                XmlEvent::EndElement { name, .. } if name.local_name == "properties" => {
                    return Ok(state);
                }
                XmlEvent::StartElement { name, .. } => {
                    state.insert(name.local_name.clone(), Self::string_element(parser)?);
                }
                _ => (),
            }
        }
    }
}

#[derive(Debug, Default, Clone)]
struct ArtifactState {
    group_id: Option<GroupId>,
    artifact_id: Option<ArtifactId>,
    version: Option<Version>,
    extension: Option<String>,
    classifier: Option<Classifier>,
}

impl ArtifactState {
    fn to_artifact(&self) -> Result<Artifact, PomParserError> {
        Ok(Artifact {
            group_id: self
                .group_id
                .clone()
                .ok_or(PomParserError::Unexpected(String::from("Missing groupId")))?,
            artifact_id: self.artifact_id.clone().ok_or(PomParserError::Unexpected(
                String::from("Missing artifactId"),
            ))?,
            version: self.version.clone(),
            extension: self.extension.clone(),
            classifier: self.classifier.clone(),
        })
    }
}

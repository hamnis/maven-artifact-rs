use crate::artifact::Artifact;
use crate::{ArtifactId, Classifier, GroupId, Version};
use std::collections::HashMap;
use std::io::{BufReader, Cursor, Read, Seek};
use thiserror::Error;
use xml::EventReader;
use xml::reader::XmlEvent;

#[derive(Debug, Clone)]
struct Dependency {
    artifact: Artifact,
    scope: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct DependencyManagement {
    dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone)]
struct Project {
    artifact: Artifact,
    parent: Option<Artifact>,
    dependency_management: DependencyManagement,
    dependencies: Vec<Dependency>,
    properties: HashMap<String, String>,
}

impl Project {
    pub fn new(artifact: Artifact) -> Self {
        Project {
            artifact,
            parent: Option::default(),
            dependency_management: DependencyManagement::default(),
            dependencies: Vec::default(),
            properties: HashMap::default(),
        }
    }
}

#[derive(Error, Debug)]
pub enum PomParserError {
    #[error("{0} IO error while parsing")]
    IO(#[from] std::io::Error),
    #[error("{0} XML error while parsing")]
    Xml(#[from] xml::reader::Error),
    #[error("{0} Unexpected XML error while parsing")]
    Unexpected(String),
}

struct PomParser {}

impl PomParser {
    pub fn from_str(input: &str) -> Result<Project, PomParserError> {
        Self::parse(Cursor::new(input))
    }
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn parse_full() {
        let pom = r###"
            <project xmlns="http://maven.apache.org/POM/4.0.0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
      xsi:schemaLocation="http://maven.apache.org/POM/4.0.0 http://maven.apache.org/xsd/maven-4.0.0.xsd">
      <modelVersion>4.0.0</modelVersion>
      <groupId>com.mycompany.app</groupId>
      <artifactId>my-app</artifactId>
      <version>1.0-SNAPSHOT</version>
      <name>my-app</name>
      <!-- FIXME change it to the project's website -->
      <url>http://www.example.com</url>
      <properties>
        <project.build.sourceEncoding>UTF-8</project.build.sourceEncoding>
        <maven.compiler.release>17</maven.compiler.release>
      </properties>
      <dependencyManagement>
        <dependencies>
          <dependency>
            <groupId>org.junit</groupId>
            <artifactId>junit-bom</artifactId>
            <version>5.11.0</version>
            <type>pom</type>
            <scope>import</scope>
          </dependency>
        </dependencies>
      </dependencyManagement>
      <dependencies>
        <dependency>
          <groupId>org.junit.jupiter</groupId>
          <artifactId>junit-jupiter-api</artifactId>
          <scope>test</scope>
        </dependency>
        <!-- Optionally: parameterized tests support -->
        <dependency>
          <groupId>org.junit.jupiter</groupId>
          <artifactId>junit-jupiter-params</artifactId>
          <scope>test</scope>
        </dependency>
      </dependencies>
      <build>
        <pluginManagement><!-- lock down plugins versions to avoid using Maven defaults (may be moved to parent pom) -->
           ... lots of helpful plugins
        </pluginManagement>
      </build>
    </project>
        "###;

        let parsed = PomParser::from_str(pom);
        println!("{:?}", parsed);
        assert!(parsed.is_ok());
    }
}

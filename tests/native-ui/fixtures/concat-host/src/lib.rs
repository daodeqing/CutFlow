//! Test boundary matching the media/session APIs used by the real coordinator.
pub mod media {
    #[derive(Clone)]pub struct MediaSummary{pub path:String}
    impl MediaSummary{pub fn to_new_media(&self)->concat_project::NewMedia{concat_project::NewMedia{path:self.path.clone()}}}
    pub fn probe(path:&str)->Result<MediaSummary,String>{
        if !std::path::Path::new(path).is_file(){return Err("fixture: file missing".into());}
        Ok(MediaSummary{path:path.into()})
    }
}
pub mod projects {
    pub struct ProjectInfo{pub path:String}
    pub fn open(path:&str)->Result<ProjectInfo,String>{
        if !std::path::Path::new(path).is_dir(){return Err("fixture: project missing".into());}
        Ok(ProjectInfo{path:path.into()})
    }
}
pub struct Session{pub path:String,pub project:concat_project::Project}
impl Session{
    pub fn path(&self)->&str{&self.path}
    pub fn save(&mut self,_:Option<&str>)->Result<(),String>{
        if std::path::Path::new(&self.path).join("fail-save").exists(){return Err("fixture: save refused".into());}
        Ok(())
    }
}
